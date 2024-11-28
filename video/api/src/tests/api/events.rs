use std::time::Duration;

use binary_helper::global::{GlobalConfig, GlobalNats};
use futures_util::StreamExt;
use pb::scuffle::video::v1::types::Event;
use pb::scuffle::video::v1::{events_fetch_request, EventsAckRequest, EventsFetchRequest};
use prost::Message;
use ulid::Ulid;
use video_common::keys::event_subject;

use crate::config::{ApiConfig, EventsConfig};
use crate::tests::api::utils::process_request;
use crate::tests::utils;

#[tokio::test]
async fn test_events() {
	let (global, handler, access_token) = scuffle_utilssetup(ApiConfig {
		events: EventsConfig {
			stream_name: Ulid::new().to_string(),
			fetch_request_min_delay: Duration::from_secs(0),
			..Default::default()
		},
		..Default::default()
	})
	.await;

	let mut response = process_request(
		&global,
		&access_token,
		EventsFetchRequest {
			target: events_fetch_request::Target::AccessToken.into(),
			max_delay_ms: 100,
			max_events: 1,
		},
	)
	.await
	.expect("failed to process request");

	let published_event = Event {
		event_id: Some(Ulid::new().into()),
		event: Some(pb::scuffle::video::v1::types::event::Event::AccessToken(
			pb::scuffle::video::v1::types::event::AccessToken {
				access_token_id: Some(Ulid::new().into()),
				event: Some(pb::scuffle::video::v1::types::event::access_token::Event::Created(
					pb::scuffle::video::v1::types::event::access_token::Created {},
				)),
			},
		)),
		timestamp: chrono::Utc::now().timestamp_millis(),
	};

	global
		.nats()
		.publish(
			event_subject(
				&global.config().events.stream_name,
				access_token.organization_id,
				events_fetch_request::Target::AccessToken,
			),
			published_event.encode_to_vec().into(),
		)
		.await
		.expect("failed to publish event");

	let received_event = response
		.next()
		.await
		.expect("failed to receive event")
		.expect("failed to receive event");

	assert_eq!(
		received_event.event.unwrap(),
		published_event,
		"received event does not match published event"
	);

	assert!(response.next().await.is_none(), "received too many events");

	process_request(
		&global,
		&access_token,
		EventsAckRequest {
			id: published_event.event_id,
			action: Some(pb::scuffle::video::v1::events_ack_request::Action::Ack(true)),
		},
	)
	.await
	.expect("failed to process request");

	scuffle_utilsteardown(global, handler).await;
}
