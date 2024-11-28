use std::sync::Arc;

use binary_helper::logging;
use scuffle_utils::context::{Context, Handler};
use scuffle_utils::database::deadpool_postgres::{ManagerConfig, PoolConfig, RecyclingMethod, Runtime};
use scuffle_utils::database::tokio_postgres::NoTls;
use scuffle_utils::database::Pool;
use scuffle_utilsgrpc::TlsSettings;

use crate::config::TranscoderConfig;

pub struct GlobalState {
	ctx: Context,
	config: TranscoderConfig,
	nats: async_nats::Client,
	jetstream: async_nats::jetstream::Context,
	db: Arc<utils::database::Pool>,
	ingest_tls: Option<TlsSettings>,
	media_store: async_nats::jetstream::object_store::ObjectStore,
	metadata_store: async_nats::jetstream::kv::Store,
}

impl binary_helper::global::GlobalCtx for GlobalState {
	fn ctx(&self) -> &Context {
		&self.ctx
	}
}

impl binary_helper::global::GlobalConfigProvider<TranscoderConfig> for GlobalState {
	fn provide_config(&self) -> &TranscoderConfig {
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

impl binary_helper::global::GlobalConfig for GlobalState {}

impl crate::global::TranscoderState for GlobalState {
	fn ingest_tls(&self) -> Option<utils::grpc::TlsSettings> {
		self.ingest_tls.clone()
	}

	fn media_store(&self) -> &async_nats::jetstream::object_store::ObjectStore {
		&self.media_store
	}

	fn metadata_store(&self) -> &async_nats::jetstream::kv::Store {
		&self.metadata_store
	}
}

pub async fn mock_global_state(config: TranscoderConfig) -> (Arc<GlobalState>, Handler) {
	let (ctx, handler) = Context::new();

	dotenvy::dotenv().ok();

	let logging_level = std::env::var("LOGGING_LEVEL").unwrap_or_else(|_| "info".to_string());

	logging::init(&logging_level, Default::default()).expect("failed to initialize logging");

	let database_uri = std::env::var("VIDEO_DATABASE_URL_TEST").expect("VIDEO_DATABASE_URL_TEST must be set");
	let nats_addr = std::env::var("NATS_ADDR").expect("NATS_URL must be set");

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

	let metadata_store = jetstream
		.create_key_value(async_nats::jetstream::kv::Config {
			bucket: config.metadata_kv_store.clone(),
			..Default::default()
		})
		.await
		.unwrap();

	let media_store = jetstream
		.create_object_store(async_nats::jetstream::object_store::Config {
			bucket: config.media_ob_store.clone(),
			..Default::default()
		})
		.await
		.unwrap();

	jetstream
		.create_stream(async_nats::jetstream::stream::Config {
			name: config.transcoder_request_subject.clone(),
			..Default::default()
		})
		.await
		.unwrap();

	let global = Arc::new(GlobalState {
		ingest_tls: config.ingest_tls.as_ref().map(|tls| {
			let cert = std::fs::read(&tls.cert).expect("failed to read redis cert");
			let key = std::fs::read(&tls.key).expect("failed to read redis key");

			let ca_cert = tls.ca_cert.as_ref().map(|ca_cert| {
				tonic::transport::Certificate::from_pem(std::fs::read(ca_cert).expect("failed to read ingest tls ca"))
			});

			TlsSettings {
				domain: tls.domain.clone(),
				identity: tonic::transport::Identity::from_pem(cert, key),
				ca_cert,
			}
		}),
		config,
		ctx,
		nats,
		jetstream,
		db,
		media_store,
		metadata_store,
	});

	(global, handler)
}
