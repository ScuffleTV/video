use scuffle_utils::http::RouteError;

use crate::subscription::SubscriptionError;

pub type Result<T, E = RouteError<EdgeError>> = std::result::Result<T, E>;

#[derive(thiserror::Error, Debug)]
pub enum EdgeError {
	#[error("failed to parse http body: {0}")]
	ParseHttpBody(hyper::Error),
	#[error("http method not allowed")]
	HttpMethodNotAllowed,
	#[error("internal server error: {0}")]
	InternalServer(&'static str),
	#[error("database error: {0}")]
	Database(#[from] scuffle_utils::database::tokio_postgres::Error),
	#[error("database pool error: {0}")]
	DatabasePool(#[from] scuffle_utils::database::deadpool_postgres::PoolError),
	#[error("json error: {0}")]
	ParseJson(#[from] serde_json::Error),
	#[error("prost error: {0}")]
	Prost(#[from] prost::DecodeError),
	#[error("subscription error: {0}")]
	Subscription(#[from] SubscriptionError),
	#[error("timeout error: {0}")]
	Timeout(#[from] tokio::time::error::Elapsed),
	#[error("io error: {0}")]
	Io(#[from] std::io::Error),
	#[error("nats error: {0}")]
	NatsObGet(#[from] async_nats::jetstream::object_store::GetError),
	#[error("nats error: {0}")]
	NatsKvGet(#[from] async_nats::jetstream::kv::EntryError),
}
