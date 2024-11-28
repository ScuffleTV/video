use std::sync::Arc;

use anyhow::Context as _;
use async_nats::jetstream::stream::{self, RetentionPolicy};
use binary_helper::config::RedisConfig;
use binary_helper::global::{setup_database, setup_nats, setup_redis, GlobalCtx, GlobalDb, GlobalNats};
use binary_helper::{bootstrap, grpc_health, grpc_server, impl_global_traits};
use scuffle_utils::context::Context;
use scuffle_utilsdataloader::DataLoader;
use tokio::select;
use video_api::config::ApiConfig;
use video_api::dataloaders;

#[derive(Debug, Clone, Default, serde::Deserialize, config::Config)]
#[serde(default)]
struct ExtConfig {
	/// The API configuration.
	api: ApiConfig,

	/// The Redis configuration.
	redis: RedisConfig,
}

impl binary_helper::config::ConfigExtention for ExtConfig {
	const APP_NAME: &'static str = "video-api";
}

type AppConfig = binary_helper::config::AppConfig<ExtConfig>;

struct GlobalState {
	ctx: Context,
	config: AppConfig,
	nats: async_nats::Client,
	jetstream: async_nats::jetstream::Context,
	db: Arc<utils::database::Pool>,
	redis: Arc<fred::clients::RedisPool>,
	access_token_loader: DataLoader<dataloaders::AccessTokenLoader>,
	recording_state_loader: DataLoader<dataloaders::RecordingStateLoader>,
	room_loader: DataLoader<dataloaders::RoomLoader>,
	events_stream: async_nats::jetstream::stream::Stream,
}

impl_global_traits!(GlobalState);

impl binary_helper::global::GlobalRedis for GlobalState {
	#[inline(always)]
	fn redis(&self) -> &Arc<fred::clients::RedisPool> {
		&self.redis
	}
}

impl binary_helper::global::GlobalConfigProvider<ApiConfig> for GlobalState {
	#[inline(always)]
	fn provide_config(&self) -> &ApiConfig {
		&self.config.extra.api
	}
}

impl video_api::global::ApiState for GlobalState {
	#[inline(always)]
	fn access_token_loader(&self) -> &DataLoader<dataloaders::AccessTokenLoader> {
		&self.access_token_loader
	}

	#[inline(always)]
	fn recording_state_loader(&self) -> &DataLoader<dataloaders::RecordingStateLoader> {
		&self.recording_state_loader
	}

	#[inline(always)]
	fn room_loader(&self) -> &DataLoader<dataloaders::RoomLoader> {
		&self.room_loader
	}

	#[inline(always)]
	fn events_stream(&self) -> &async_nats::jetstream::stream::Stream {
		&self.events_stream
	}
}

impl binary_helper::Global<AppConfig> for GlobalState {
	async fn new(ctx: Context, config: AppConfig) -> anyhow::Result<Self> {
		let (nats, jetstream) = setup_nats(&config.name, &config.nats).await?;
		let db = setup_database(&config.database).await?;
		let redis = setup_redis(&config.extra.redis).await?;

		let access_token_loader = dataloaders::AccessTokenLoader::new(db.clone());
		let recording_state_loader = dataloaders::RecordingStateLoader::new(db.clone());
		let room_loader = dataloaders::RoomLoader::new(db.clone());

		scuffle_utilsratelimiter::load_rate_limiter_script(&*redis)
			.await
			.context("failed to load rate limiter script")?;

		let events_stream = jetstream
			.get_or_create_stream(stream::Config {
				name: config.extra.api.events.stream_name.clone(),
				subjects: vec![format!("{}.>", config.extra.api.events.stream_name)],
				retention: RetentionPolicy::WorkQueue,
				max_age: config.extra.api.events.nats_stream_message_max_age,
				..Default::default()
			})
			.await
			.context("failed to create event stream")?;

		Ok(Self {
			ctx,
			config,
			nats,
			jetstream,
			db,
			redis,
			access_token_loader,
			recording_state_loader,
			room_loader,
			events_stream,
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

			let router = video_api::grpc::add_routes(&global, router);

			router.serve_with_shutdown(global.config.grpc.bind_address, async {
				global.ctx().done().await;
			})
		};

		let api_future = video_api::api::run(global.clone());

		select! {
			r = grpc_future => r.context("grpc server stopped unexpectedly")?,
			r = api_future => r.context("api server stopped unexpectedly")?,
		}

		Ok(())
	})
	.await
	{
		tracing::error!("{:#}", err);
		std::process::exit(1);
	}
}
