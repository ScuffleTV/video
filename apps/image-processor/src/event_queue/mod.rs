use scuffle_image_processor_proto::EventCallback;

use self::http::{HttpEventQueue, HttpEventQueueError};
use self::nats::{NatsEventQueue, NatsEventQueueError};
use self::redis::{RedisEventQueue, RedisEventQueueError};
use crate::config::EventQueueConfig;

pub mod http;
pub mod nats;
pub mod redis;

#[derive(Debug, thiserror::Error)]
pub enum EventQueueError {
	#[error("nats: {0}")]
	Nats(#[from] NatsEventQueueError),
	#[error("http: {0}")]
	Http(#[from] HttpEventQueueError),
	#[error("redis: {0}")]
	Redis(#[from] RedisEventQueueError),
}

const PROTOBUF_CONTENT_TYPE: &str = "application/protobuf; proto=scuffle.image_processor.EventCallback";

pub trait EventQueue {
	fn name(&self) -> &str;

	fn publish(
		&self,
		topic: &str,
		data: EventCallback,
	) -> impl std::future::Future<Output = Result<(), EventQueueError>> + Send;

	fn healthy(&self) -> impl std::future::Future<Output = bool> + Send {
		async { true }
	}
}

#[derive(Debug)]
pub enum AnyEventQueue {
	Nats(NatsEventQueue),
	Http(HttpEventQueue),
	Redis(RedisEventQueue),
}

impl EventQueue for AnyEventQueue {
	fn name(&self) -> &str {
		match self {
			AnyEventQueue::Nats(queue) => queue.name(),
			AnyEventQueue::Http(queue) => queue.name(),
			AnyEventQueue::Redis(queue) => queue.name(),
		}
	}

	async fn publish(&self, topic: &str, data: EventCallback) -> Result<(), EventQueueError> {
		match self {
			AnyEventQueue::Nats(queue) => queue.publish(topic, data).await,
			AnyEventQueue::Http(queue) => queue.publish(topic, data).await,
			AnyEventQueue::Redis(queue) => queue.publish(topic, data).await,
		}
	}
}

pub async fn build_event_queue(config: &EventQueueConfig) -> Result<AnyEventQueue, EventQueueError> {
	match config {
		EventQueueConfig::Nats(nats) => Ok(AnyEventQueue::Nats(NatsEventQueue::new(nats).await?)),
		EventQueueConfig::Redis(redis) => Ok(AnyEventQueue::Redis(RedisEventQueue::new(redis).await?)),
		EventQueueConfig::Http(http) => Ok(AnyEventQueue::Http(HttpEventQueue::new(http).await?)),
	}
}
