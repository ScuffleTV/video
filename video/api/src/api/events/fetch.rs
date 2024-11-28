use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use async_nats::jetstream::consumer::{self, AckPolicy, DeliverPolicy};
use fred::interfaces::KeysInterface;
use fred::types::Expiration;
use futures_util::StreamExt;
use pb::ext::UlidExt;
use pb::scuffle::video::v1::events_fetch_request::Target;
use pb::scuffle::video::v1::types::access_token_scope::Permission;
use pb::scuffle::video::v1::types::{AccessTokenScope, Resource};
use pb::scuffle::video::v1::{EventsFetchRequest, EventsFetchResponse};
use prost::Message;
use video_common::database::AccessToken;
use video_common::keys::event_subject;

use super::utils::ack_key;
use crate::api::utils::{impl_request_scopes, AccessTokenExt, ApiRequest, RequiredScope};
use crate::config::ApiConfig;
use crate::global::ApiGlobal;
use crate::ratelimit::RateLimitResource;

impl_request_scopes!(
	EventsFetchRequest,
	(),
	(Resource::Event, Permission::Read),
	RateLimitResource::EventsFetch
);

pub type Stream = Pin<Box<dyn futures_util::Stream<Item = tonic::Result<EventsFetchResponse>> + Send>>;

impl ApiRequest<Stream> for tonic::Request<EventsFetchRequest> {
	async fn process<G: ApiGlobal>(
		&self,
		global: &Arc<G>,
		access_token: &AccessToken,
	) -> tonic::Result<tonic::Response<Stream>> {
		let req = self.get_ref();
		let target = req.target();

		let config = &global.config::<ApiConfig>().events;

		let required_permissions = RequiredScope({
			let resource = match target {
				Target::AccessToken => Resource::AccessToken,
				Target::PlaybackKeyPair => Resource::PlaybackKeyPair,
				Target::Recording => Resource::Recording,
				Target::RecordingConfig => Resource::RecordingConfig,
				Target::Room => Resource::Room,
				Target::S3Bucket => Resource::S3Bucket,
				Target::TranscodingConfig => Resource::TranscodingConfig,
			};

			vec![AccessTokenScope {
				resource: Some(resource.into()),
				permission: vec![Permission::Events.into()],
			}]
		});

		access_token.has_scope(&required_permissions)?;

		let expires = Duration::from_millis(req.max_delay_ms as u64)
			.max(config.fetch_request_min_delay)
			.min(config.fetch_request_max_delay);
		let max_messages = (req.max_events as usize)
			.max(config.fetch_request_min_messages)
			.min(config.fetch_request_max_messages);

		let name = format!("{}-{}", access_token.organization_id.0, target.as_str_name());

		let consumer = global
			.events_stream()
			.get_or_create_consumer(
				&name,
				consumer::pull::Config {
					name: Some(name.clone()),
					deliver_policy: DeliverPolicy::All,
					ack_policy: AckPolicy::Explicit,
					filter_subject: event_subject(&config.stream_name, access_token.organization_id, target),
					ack_wait: config.nats_stream_message_lease_duration,
					..Default::default()
				},
			)
			.await
			.map_err(|err| {
				tracing::error!(err = %err, "failed to create events consumer");
				tonic::Status::internal("failed to create events consumer")
			})?;

		let mut messages = consumer
			.fetch()
			.expires(expires)
			.max_messages(max_messages)
			.messages()
			.await
			.map_err(|err| {
				tracing::error!(err = %err, "failed to subscribe to events consumer");
				tonic::Status::internal("failed to subscribe to events consumer")
			})?;

		let organization_id = access_token.organization_id;
		let global = global.clone();

		let lease_duration = config.nats_stream_message_lease_duration.as_secs().max(1) as i64;

		Ok(tonic::Response::new(Box::pin(async_stream::stream! {
			while let Some(message) = messages.next().await {
				let message = match message {
					Ok(message) => message.message,
					Err(err) => {
						tracing::error!(err = %err, "failed to receive message from events consumer");
						yield Err(tonic::Status::internal("failed to receive message from events consumer"));
						continue;
					}
				};

				let event = match pb::scuffle::video::v1::types::Event::decode(message.payload.as_ref()) {
					Ok(event) => event,
					Err(err) => {
						tracing::error!(err = %err, "failed to decode event");
						continue;
					}
				};

				let id = event.event_id.into_ulid();
				let Some(reply) = message.reply else {
					tracing::error!("missing reply subject");
					continue;
				};

				global.redis().set(&ack_key(organization_id, id), reply.as_ref(), Some(Expiration::EX(lease_duration)), None, false).await.map_err(|err| {
					tracing::error!(err = %err, "failed to set event id in redis");
					tonic::Status::internal("failed to set event id in redis")
				})?;

				yield Ok(EventsFetchResponse {
					event: Some(event),
				});
			}
		}) as Stream))
	}
}
