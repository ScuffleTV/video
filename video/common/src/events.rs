use pb::scuffle::video::v1::events_fetch_request::Target;
use pb::scuffle::video::v1::types::{event, Event};
use prost::Message;

use crate::keys::event_subject;

pub async fn emit(nats: &async_nats::Client, stream_name: &str, org_id: ulid::Ulid, target: Target, event: event::Event) {
	nats.publish(
		event_subject(stream_name, org_id, target),
		Event {
			timestamp: chrono::Utc::now().timestamp_millis(),
			event_id: Some(ulid::Ulid::new().into()),
			event: Some(event),
		}
		.encode_to_vec()
		.into(),
	)
	.await
	.map_err(|e| {
		tracing::error!(err = %e, "failed to publish event");
	})
	.ok();
}
