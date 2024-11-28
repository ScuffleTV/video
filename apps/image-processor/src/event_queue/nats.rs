use async_nats::ConnectOptions;
use prost::Message;
use scuffle_image_processor_proto::EventCallback;

use super::{EventQueue, EventQueueError, PROTOBUF_CONTENT_TYPE};
use crate::config::{MessageEncoding, NatsEventQueueConfig};

#[derive(Debug)]
pub struct NatsEventQueue {
	name: String,
	message_encoding: MessageEncoding,
	nats: async_nats::Client,
}

#[derive(Debug, thiserror::Error)]
pub enum NatsEventQueueError {
	#[error("connect: {0}")]
	Connect(#[from] async_nats::ConnectError),
	#[error("encode json: {0}")]
	EncodeJson(#[from] serde_json::Error),
	#[error("publish: {0}")]
	Publish(#[from] async_nats::PublishError),
}

impl NatsEventQueue {
	#[tracing::instrument(skip(config), name = "NatsEventQueue::new", fields(name = %config.name), err)]
	pub async fn new(config: &NatsEventQueueConfig) -> Result<Self, EventQueueError> {
		let nats = async_nats::connect_with_options(&config.servers, {
			let options = ConnectOptions::default();

			if let Some(username) = &config.username {
				options.user_and_password(username.clone(), config.password.clone().unwrap_or_default())
			} else {
				options
			}
		})
		.await
		.map_err(NatsEventQueueError::from)?;

		Ok(Self {
			name: config.name.clone(),
			message_encoding: config.message_encoding,
			nats,
		})
	}
}

impl EventQueue for NatsEventQueue {
	fn name(&self) -> &str {
		&self.name
	}

	#[tracing::instrument(skip(self), name = "NatsEventQueue::publish", err)]
	async fn publish(&self, topic: &str, data: EventCallback) -> Result<(), EventQueueError> {
		let mut header_map = async_nats::HeaderMap::new();

		let payload = if self.message_encoding == MessageEncoding::Protobuf {
			header_map.insert("Content-Type", PROTOBUF_CONTENT_TYPE);
			data.encode_to_vec()
		} else {
			header_map.insert("Content-Type", "application/json");
			serde_json::to_string(&data)
				.map_err(NatsEventQueueError::EncodeJson)?
				.into_bytes()
		}
		.into();

		self.nats
			.publish_with_headers(topic.to_owned(), header_map, payload)
			.await
			.map_err(NatsEventQueueError::Publish)?;

		Ok(())
	}

	async fn healthy(&self) -> bool {
		matches!(self.nats.connection_state(), async_nats::connection::State::Connected)
	}
}
