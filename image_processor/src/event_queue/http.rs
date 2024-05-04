use prost::Message;
use scuffle_image_processor_proto::EventCallback;
use url::Url;

use super::{EventQueue, EventQueueError, PROTOBUF_CONTENT_TYPE};
use crate::config::HttpEventQueueConfig;

#[derive(Debug)]
pub struct HttpEventQueue {
	name: String,
	url: Url,
	client: reqwest::Client,
	semaphore: Option<tokio::sync::Semaphore>,
	allow_protobuf: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum HttpEventQueueError {
	#[error("reqwest: {0}")]
	Reqwest(#[from] reqwest::Error),
	#[error("invalid header name")]
	InvalidHeaderName(#[from] reqwest::header::InvalidHeaderName),
	#[error("invalid header value")]
	InvalidHeaderValue(#[from] reqwest::header::InvalidHeaderValue),
}

impl HttpEventQueue {
	#[tracing::instrument(skip(config), name = "HttpEventQueue::new", fields(name = %config.name), err)]
	pub async fn new(config: &HttpEventQueueConfig) -> Result<Self, HttpEventQueueError> {
		tracing::debug!("setting up http event queue");
		Ok(Self {
			name: config.name.clone(),
			client: {
				let mut builder = reqwest::Client::builder();

				if let Some(timeout) = config.timeout {
					builder = builder.timeout(timeout);
				}

				if config.allow_insecure {
					builder = builder.danger_accept_invalid_certs(true);
				}

				let mut headers = reqwest::header::HeaderMap::new();

				for (key, value) in &config.headers {
					headers.insert(
						key.parse::<reqwest::header::HeaderName>()?,
						value.parse::<reqwest::header::HeaderValue>()?,
					);
				}

				builder = builder.default_headers(headers);

				builder.build().map_err(|e| HttpEventQueueError::Reqwest(e))?
			},
			url: config.url.clone(),
			allow_protobuf: config.allow_protobuf,
			semaphore: config.max_connections.map(|max| tokio::sync::Semaphore::new(max)),
		})
	}
}

impl EventQueue for HttpEventQueue {
	fn name(&self) -> &str {
		&self.name
	}

	#[tracing::instrument(skip(self), name = "HttpEventQueue::publish", fields(name = %self.name))]
	async fn publish(&self, topic: &str, data: EventCallback) -> Result<(), EventQueueError> {
		let _permit = if let Some(semaphore) = &self.semaphore {
			Some(semaphore.acquire().await)
		} else {
			None
		};

		let mut req = self.client.post(self.url.clone()).header("X-Topic", topic);

		if self.allow_protobuf {
			req = req.header("Content-Type", PROTOBUF_CONTENT_TYPE).body(data.encode_to_vec());
		} else {
			req = req.json(&data);
		}

		req.send()
			.await
			.map_err(HttpEventQueueError::Reqwest)?
			.error_for_status()
			.map_err(HttpEventQueueError::Reqwest)?;

		Ok(())
	}
}
