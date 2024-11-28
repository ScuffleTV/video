use std::sync::Arc;

use async_nats::jetstream::stream::{self, RetentionPolicy};
use binary_helper::logging;
use fred::interfaces::ClientLike;
use postgres_from_row::tokio_postgres::NoTls;
use scuffle_utils::context::{Context, Handler};
use scuffle_utils::database::deadpool_postgres::{ManagerConfig, PoolConfig, RecyclingMethod, Runtime};
use scuffle_utils::database::Pool;
use scuffle_utils::prelude::FutureTimeout;
use scuffle_utilsdataloader::DataLoader;

use crate::config::ApiConfig;
use crate::dataloaders;

pub struct GlobalState {
	ctx: Context,
	config: ApiConfig,
	nats: async_nats::Client,
	jetstream: async_nats::jetstream::Context,
	db: Arc<utils::database::Pool>,
	redis: Arc<fred::clients::RedisPool>,

	access_token_loader: DataLoader<dataloaders::AccessTokenLoader>,
	recording_state_loader: DataLoader<dataloaders::RecordingStateLoader>,
	room_loader: DataLoader<dataloaders::RoomLoader>,

	events_stream: async_nats::jetstream::stream::Stream,
}

impl binary_helper::global::GlobalCtx for GlobalState {
	fn ctx(&self) -> &Context {
		&self.ctx
	}
}

impl binary_helper::global::GlobalConfigProvider<ApiConfig> for GlobalState {
	fn provide_config(&self) -> &ApiConfig {
		&self.config
	}
}

impl binary_helper::global::GlobalNats for GlobalState {
	fn nats(&self) -> &async_nats::Client {
		&self.nats
	}

	fn jetstream(&self) -> &async_nats::jetstream::Context {
		&self.jetstream
	}
}

impl binary_helper::global::GlobalDb for GlobalState {
	fn db(&self) -> &Arc<utils::database::Pool> {
		&self.db
	}
}

impl binary_helper::global::GlobalRedis for GlobalState {
	fn redis(&self) -> &Arc<fred::clients::RedisPool> {
		&self.redis
	}
}

impl binary_helper::global::GlobalConfig for GlobalState {}

impl crate::global::ApiState for GlobalState {
	fn access_token_loader(&self) -> &DataLoader<dataloaders::AccessTokenLoader> {
		&self.access_token_loader
	}

	fn recording_state_loader(&self) -> &DataLoader<dataloaders::RecordingStateLoader> {
		&self.recording_state_loader
	}

	fn room_loader(&self) -> &DataLoader<dataloaders::RoomLoader> {
		&self.room_loader
	}

	fn events_stream(&self) -> &async_nats::jetstream::stream::Stream {
		&self.events_stream
	}
}

pub async fn mock_global_state(config: ApiConfig) -> (Arc<GlobalState>, Handler) {
	let (ctx, handler) = Context::new();

	dotenvy::dotenv().ok();

	let logging_level = std::env::var("LOGGING_LEVEL").unwrap_or_else(|_| "info".to_string());

	logging::init(&logging_level, Default::default()).expect("failed to initialize logging");

	let database_uri = std::env::var("VIDEO_DATABASE_URL_TEST").expect("VIDEO_DATABASE_URL_TEST must be set");
	let nats_addr = std::env::var("NATS_ADDR").expect("NATS_URL must be set");
	let redis_url = std::env::var("REDIS_ADDR")
		.map(|addr| format!("redis://{addr}"))
		.unwrap_or_else(|_| std::env::var("REDIS_URL").expect("REDIS_URL and REDIS_ADDR are not set"));

	let nats = async_nats::connect(&nats_addr).await.expect("failed to connect to nats");
	let jetstream = async_nats::jetstream::new(nats.clone());

	let db = Arc::new(
		Pool::builder(utils::database::deadpool_postgres::Manager::from_config(
			database_uri.parse().unwrap(),
			NoTls,
			ManagerConfig {
				recycling_method: RecyclingMethod::Fast,
			},
		))
		.config(PoolConfig::default())
		.runtime(Runtime::Tokio1)
		.build()
		.expect("failed to create pool"),
	);

	let redis = Arc::new(
		fred::clients::RedisPool::new(fred::types::RedisConfig::from_url(&redis_url).unwrap(), None, None, None, 1).unwrap(),
	);

	redis.connect();

	redis
		.wait_for_connect()
		.timeout(std::time::Duration::from_secs(5))
		.await
		.expect("failed to connect to redis")
		.expect("failed to connect to redis");

	scuffle_utilsratelimiter::load_rate_limiter_script(&*redis)
		.await
		.expect("failed to load rate limiter script");

	let events_stream = jetstream
		.get_or_create_stream(stream::Config {
			name: config.events.stream_name.clone(),
			subjects: vec![format!("{}.>", config.events.stream_name)],
			retention: RetentionPolicy::WorkQueue,
			max_age: config.events.nats_stream_message_max_age,
			..Default::default()
		})
		.await
		.expect("failed to create event stream");

	let access_token_loader = dataloaders::AccessTokenLoader::new(db.clone());
	let recording_state_loader = dataloaders::RecordingStateLoader::new(db.clone());
	let room_loader = dataloaders::RoomLoader::new(db.clone());

	let global = Arc::new(GlobalState {
		config,
		ctx,
		nats,
		jetstream,
		db,
		access_token_loader,
		recording_state_loader,
		room_loader,
		redis,
		events_stream,
	});

	(global, handler)
}
