use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Result};
use async_nats::jetstream::consumer::pull::Config;
use async_nats::jetstream::consumer::DeliverPolicy;
use async_nats::jetstream::stream::RetentionPolicy;
use futures::StreamExt;
use scuffle_utils::context::ContextExt;
use tokio_util::sync::CancellationToken;

use crate::config::TranscoderConfig;
use crate::global::TranscoderGlobal;
use crate::transcoder::job::handle_message;

pub(crate) mod job;

pub async fn run<G: TranscoderGlobal>(global: Arc<G>) -> Result<()> {
	let config = global.config::<TranscoderConfig>();

	let stream = global
		.jetstream()
		.get_or_create_stream(async_nats::jetstream::stream::Config {
			name: config.transcoder_request_subject.clone(),
			max_age: Duration::from_secs(60 * 2), // 2 minutes max age
			retention: RetentionPolicy::WorkQueue,
			subjects: vec![config.transcoder_request_subject.clone()],
			storage: async_nats::jetstream::stream::StorageType::Memory,
			..Default::default()
		})
		.await?;

	let consumer = stream
		.get_or_create_consumer(
			"transcoder",
			Config {
				name: Some("transcoder".to_string()),
				filter_subject: config.transcoder_request_subject.clone(),
				max_deliver: 3,
				deliver_policy: DeliverPolicy::All,
				..Default::default()
			},
		)
		.await?;

	let mut messages = consumer.messages().await?;

	let shutdown_token = CancellationToken::new();
	let child_token = shutdown_token.child_token();
	let _drop_guard = shutdown_token.clone().drop_guard();

	while let Ok(m) = messages.next().context(global.ctx()).await {
		let m = match m {
			Some(Ok(m)) => m,
			Some(Err(e)) => {
				tracing::error!("error receiving message: {}", e);
				continue;
			}
			None => {
				bail!("nats stream closed");
			}
		};

		tokio::spawn(handle_message(global.clone(), m, child_token.clone()));
	}

	drop(messages);
	drop(consumer);

	tokio::time::sleep(Duration::from_millis(100)).await;

	global.nats().flush().await?;

	Ok(())
}
