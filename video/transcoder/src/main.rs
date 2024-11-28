use std::sync::Arc;
use std::time::Duration;

use anyhow::Context as _;
use async_nats::jetstream::stream::StorageType;
use binary_helper::global::{setup_database, setup_nats, GlobalCtx, GlobalDb, GlobalNats};
use binary_helper::{bootstrap, grpc_health, grpc_server, impl_global_traits};
use scuffle_utils::context::Context;
use scuffle_utilsgrpc::TlsSettings;
use tokio::select;
use video_transcoder::config::TranscoderConfig;

#[derive(Debug, Clone, Default, serde::Deserialize, config::Config)]
#[serde(default)]
struct ExtConfig {
	/// The Transcoder configuration.
	transcoder: TranscoderConfig,
}

impl binary_helper::config::ConfigExtention for ExtConfig {
	const APP_NAME: &'static str = "video-transcoder";
}

type AppConfig = binary_helper::config::AppConfig<ExtConfig>;

struct GlobalState {
	ctx: Context,
	config: AppConfig,
	nats: async_nats::Client,
	jetstream: async_nats::jetstream::Context,
	db: Arc<utils::database::Pool>,
	metadata_store: async_nats::jetstream::kv::Store,
	media_store: async_nats::jetstream::object_store::ObjectStore,
	ingest_tls: Option<utils::grpc::TlsSettings>,
}

impl_global_traits!(GlobalState);

impl binary_helper::global::GlobalConfigProvider<TranscoderConfig> for GlobalState {
	#[inline(always)]
	fn provide_config(&self) -> &TranscoderConfig {
		&self.config.extra.transcoder
	}
}

impl video_transcoder::global::TranscoderState for GlobalState {
	#[inline(always)]
	fn metadata_store(&self) -> &async_nats::jetstream::kv::Store {
		&self.metadata_store
	}

	#[inline(always)]
	fn media_store(&self) -> &async_nats::jetstream::object_store::ObjectStore {
		&self.media_store
	}

	#[inline(always)]
	fn ingest_tls(&self) -> Option<utils::grpc::TlsSettings> {
		self.ingest_tls.clone()
	}
}

impl binary_helper::Global<AppConfig> for GlobalState {
	async fn new(ctx: Context, config: AppConfig) -> anyhow::Result<Self> {
		let (nats, jetstream) = setup_nats(&config.name, &config.nats).await?;
		let db = setup_database(&config.database).await?;

		let metadata_store = match jetstream.get_key_value(&config.extra.transcoder.metadata_kv_store).await {
			Ok(metadata_store) => metadata_store,
			Err(err) => {
				tracing::warn!("failed to get metadata kv store: {}", err);

				jetstream
					.create_key_value(async_nats::jetstream::kv::Config {
						bucket: config.extra.transcoder.metadata_kv_store.clone(),
						max_age: Duration::from_secs(60), // 1 minutes max age
						storage: StorageType::Memory,
						..Default::default()
					})
					.await
					.context("failed to create metadata kv store")?
			}
		};

		let media_store = match jetstream.get_object_store(&config.extra.transcoder.media_ob_store).await {
			Ok(media_store) => media_store,
			Err(err) => {
				tracing::warn!("failed to get media object store: {}", err);

				jetstream
					.create_object_store(async_nats::jetstream::object_store::Config {
						bucket: config.extra.transcoder.media_ob_store.clone(),
						max_age: Duration::from_secs(60), // 1 minutes max age
						storage: StorageType::File,
						..Default::default()
					})
					.await
					.context("failed to create media object store")?
			}
		};

		let ingest_tls = if let Some(tls) = &config.extra.transcoder.ingest_tls {
			let cert = tokio::fs::read(&tls.cert).await.context("failed to read ingest tls cert")?;
			let key = tokio::fs::read(&tls.key).await.context("failed to read ingest tls key")?;

			let ca_cert = if let Some(ca_cert) = &tls.ca_cert {
				Some(tonic::transport::Certificate::from_pem(
					tokio::fs::read(&ca_cert).await.context("failed to read ingest tls ca")?,
				))
			} else {
				None
			};

			Some(TlsSettings {
				domain: tls.domain.clone(),
				ca_cert,
				identity: tonic::transport::Identity::from_pem(cert, key),
			})
		} else {
			None
		};

		Ok(Self {
			ctx,
			config,
			nats,
			jetstream,
			db,
			metadata_store,
			media_store,
			ingest_tls,
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

			let router = video_transcoder::grpc::add_routes(&global, router);

			router.serve_with_shutdown(global.config.grpc.bind_address, async {
				global.ctx().done().await;
			})
		};

		let transcoder_future = video_transcoder::transcoder::run(global.clone());

		select! {
			r = grpc_future => r.context("grpc server stopped unexpectedly")?,
			r = transcoder_future => r.context("transcoder server stopped unexpectedly")?,
		}

		Ok(())
	})
	.await
	{
		tracing::error!("{:#}", err);
		std::process::exit(1);
	}
}
