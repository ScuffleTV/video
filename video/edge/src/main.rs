use std::sync::Arc;
use std::time::Duration;

use anyhow::Context as _;
use async_nats::jetstream::stream::StorageType;
use binary_helper::global::{setup_database, setup_nats, GlobalCtx, GlobalDb, GlobalNats};
use binary_helper::{bootstrap, grpc_health, grpc_server, impl_global_traits};
use scuffle_utils::context::Context;
use tokio::select;
use video_edge::config::EdgeConfig;
use video_edge::global::EdgeState;
use video_edge::subscription;

#[derive(Debug, Clone, Default, serde::Deserialize, config::Config)]
#[serde(default)]
struct ExtConfig {
	/// The Edge configuration.
	edge: EdgeConfig,
}

impl binary_helper::config::ConfigExtention for ExtConfig {
	const APP_NAME: &'static str = "video-edge";
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
	subscriber: subscription::SubscriptionManager,
}

impl_global_traits!(GlobalState);

impl binary_helper::global::GlobalConfigProvider<EdgeConfig> for GlobalState {
	#[inline(always)]
	fn provide_config(&self) -> &EdgeConfig {
		&self.config.extra.edge
	}
}

impl video_edge::global::EdgeState for GlobalState {
	#[inline(always)]
	fn metadata_store(&self) -> &async_nats::jetstream::kv::Store {
		&self.metadata_store
	}

	#[inline(always)]
	fn media_store(&self) -> &async_nats::jetstream::object_store::ObjectStore {
		&self.media_store
	}

	#[inline(always)]
	fn subscriber(&self) -> &subscription::SubscriptionManager {
		&self.subscriber
	}
}

impl binary_helper::Global<AppConfig> for GlobalState {
	async fn new(ctx: Context, config: AppConfig) -> anyhow::Result<Self> {
		let (nats, jetstream) = setup_nats(&config.name, &config.nats).await?;
		let db = setup_database(&config.database).await?;

		let metadata_store = match jetstream.get_key_value(&config.extra.edge.metadata_kv_store).await {
			Ok(metadata_store) => metadata_store,
			Err(err) => {
				tracing::warn!("failed to get metadata kv store: {}", err);

				jetstream
					.create_key_value(async_nats::jetstream::kv::Config {
						bucket: config.extra.edge.metadata_kv_store.clone(),
						max_age: Duration::from_secs(60), // 1 minutes max age
						storage: StorageType::Memory,
						..Default::default()
					})
					.await
					.context("failed to create metadata kv store")?
			}
		};

		let media_store = match jetstream.get_object_store(&config.extra.edge.media_ob_store).await {
			Ok(media_store) => media_store,
			Err(err) => {
				tracing::warn!("failed to get media object store: {}", err);

				jetstream
					.create_object_store(async_nats::jetstream::object_store::Config {
						bucket: config.extra.edge.media_ob_store.clone(),
						max_age: Duration::from_secs(60), // 1 minutes max age
						storage: StorageType::File,
						..Default::default()
					})
					.await
					.context("failed to create media object store")?
			}
		};

		Ok(Self {
			ctx,
			config,
			nats,
			jetstream,
			db,
			metadata_store,
			media_store,
			subscriber: subscription::SubscriptionManager::default(),
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

			let router = video_edge::grpc::add_routes(&global, router);

			router.serve_with_shutdown(global.config.grpc.bind_address, async {
				global.ctx().done().await;
			})
		};

		let edge_future = video_edge::edge::run(global.clone());

		let subscription_manager_future = global.subscriber().run(global.ctx(), global.metadata_store());

		select! {
			r = grpc_future => r.context("grpc server stopped unexpectedly")?,
			r = edge_future => r.context("edge server stopped unexpectedly")?,
			r = subscription_manager_future => r.context("subscription manager stopped unexpectedly")?,
		}

		Ok(())
	})
	.await
	{
		tracing::error!("{:#}", err);
		std::process::exit(1);
	}
}
