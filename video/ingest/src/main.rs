use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;

use anyhow::Context as _;
use binary_helper::global::{setup_database, setup_nats, GlobalCtx, GlobalDb, GlobalNats};
use binary_helper::{bootstrap, grpc_health, grpc_server, impl_global_traits};
use scuffle_utils::context::Context;
use tokio::select;
use tokio::sync::{mpsc, Mutex};
use ulid::Ulid;
use video_ingest::config::IngestConfig;
use video_ingest::global::IncomingTranscoder;

#[derive(Debug, Clone, Default, serde::Deserialize, config::Config)]
#[serde(default)]
struct ExtConfig {
	/// The Ingest configuration.
	ingest: IngestConfig,
}

impl binary_helper::config::ConfigExtention for ExtConfig {
	const APP_NAME: &'static str = "video-ingest";
}

type AppConfig = binary_helper::config::AppConfig<ExtConfig>;

struct GlobalState {
	ctx: Context,
	config: AppConfig,
	nats: async_nats::Client,
	jetstream: async_nats::jetstream::Context,
	db: Arc<utils::database::Pool>,

	requests: Mutex<HashMap<Ulid, mpsc::Sender<IncomingTranscoder>>>,
}

impl_global_traits!(GlobalState);

impl binary_helper::global::GlobalConfigProvider<IngestConfig> for GlobalState {
	#[inline(always)]
	fn provide_config(&self) -> &IngestConfig {
		&self.config.extra.ingest
	}
}

impl video_ingest::global::IngestState for GlobalState {
	fn requests(&self) -> &Mutex<HashMap<Ulid, mpsc::Sender<IncomingTranscoder>>> {
		&self.requests
	}
}

impl binary_helper::Global<AppConfig> for GlobalState {
	async fn new(ctx: Context, mut config: AppConfig) -> anyhow::Result<Self> {
		let (nats, jetstream) = setup_nats(&config.name, &config.nats).await?;
		let db = setup_database(&config.database).await?;

		if config.grpc.bind_address.port() == 0 {
			// Get a random port
			let port = std::net::TcpListener::bind((config.grpc.bind_address.ip(), 0))?
				.local_addr()?
				.port();

			config.grpc.bind_address.set_port(port);
		}

		if config.extra.ingest.grpc_advertise_address.is_empty() {
			config.extra.ingest.grpc_advertise_address = format!(
				"http{}://{}:{}",
				if config.grpc.tls.is_some() { "s" } else { "" },
				if config.grpc.bind_address.ip().is_unspecified() {
					default_net::interface::get_local_ipaddr().unwrap_or(IpAddr::from([127, 0, 0, 1]))
				} else {
					config.grpc.bind_address.ip()
				},
				config.grpc.bind_address.port(),
			);
		} else if !config.extra.ingest.grpc_advertise_address.starts_with("http://")
			&& !config.extra.ingest.grpc_advertise_address.starts_with("https://")
		{
			config.extra.ingest.grpc_advertise_address = format!(
				"http{}://{}",
				if config.grpc.tls.is_some() { "s" } else { "" },
				config.extra.ingest.grpc_advertise_address
			);
		}

		tracing::info!("grpc advertise address: {}", config.extra.ingest.grpc_advertise_address);

		Ok(Self {
			ctx,
			config,
			nats,
			jetstream,
			db,
			requests: Mutex::new(HashMap::new()),
		})
	}
}

#[tokio::main]
pub async fn main() {
	if let Err(err) = bootstrap::<AppConfig, GlobalState, _>(|global| async move {
		let grpc_future = {
			let mut server = grpc_server(&global.config.grpc)
				.await
				.context("failed to create grpc server")?;
			let router = server.add_service(grpc_health::HealthServer::new(&global, |global, _| async move {
				!global.db().is_closed() && global.nats().connection_state() == async_nats::connection::State::Connected
			}));

			let router = video_ingest::grpc::add_routes(&global, router);

			router.serve_with_shutdown(global.config.grpc.bind_address, async {
				global.ctx().done().await;
			})
		};

		let ingest_future = video_ingest::ingest::run(global.clone());

		select! {
			r = grpc_future => r.context("grpc server stopped unexpectedly")?,
			r = ingest_future => r.context("ingest server stopped unexpectedly")?,
		}

		Ok(())
	})
	.await
	{
		tracing::error!("{:#}", err);
		std::process::exit(1);
	}
}
