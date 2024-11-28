use std::sync::Arc;
use std::time::Duration;

use async_nats::jetstream::AckKind;
use fred::interfaces::KeysInterface;
use pb::ext::UlidExt;
use pb::scuffle::video::v1::types::access_token_scope::Permission;
use pb::scuffle::video::v1::types::Resource;
use pb::scuffle::video::v1::{EventsAckRequest, EventsAckResponse};
use video_common::database::AccessToken;

use super::utils::ack_key;
use crate::api::utils::{impl_request_scopes, ApiRequest};
use crate::config::ApiConfig;
use crate::global::ApiGlobal;
use crate::ratelimit::RateLimitResource;

impl_request_scopes!(
	EventsAckRequest,
	(),
	(Resource::Event, Permission::Read),
	RateLimitResource::EventsAck
);

impl ApiRequest<EventsAckResponse> for tonic::Request<EventsAckRequest> {
	async fn process<G: ApiGlobal>(
		&self,
		global: &Arc<G>,
		access_token: &AccessToken,
	) -> tonic::Result<tonic::Response<EventsAckResponse>> {
		let req = self.get_ref();

		let config = &global.config::<ApiConfig>().events;

		let id = req.id.into_ulid();
		let key = ack_key(access_token.organization_id, id);

		let reply: Option<String> = global.redis().get(&key).await.map_err(|err| {
			tracing::error!(err = %err, "failed to get event id from redis");
			tonic::Status::internal("failed to get event id from redis")
		})?;

		let Some(reply) = reply else {
			return Err(tonic::Status::not_found("event not found"));
		};

		let ack_kind = match req.action {
			Some(pb::scuffle::video::v1::events_ack_request::Action::Ack(_)) => AckKind::Ack,
			Some(pb::scuffle::video::v1::events_ack_request::Action::Reject(_)) => AckKind::Term,
			Some(pb::scuffle::video::v1::events_ack_request::Action::RequeueDelayMs(time)) => {
				AckKind::Nak(if time > 60 * 60 * 1000 {
					return Err(tonic::Status::invalid_argument(
						"invalid requeue delay, must be in range [0 and 3600000]",
					));
				} else if time > 0 {
					Some(Duration::from_millis(time as u64))
				} else {
					None
				})
			}
			Some(pb::scuffle::video::v1::events_ack_request::Action::Reclaim(_)) => AckKind::Progress,
			None => return Err(tonic::Status::invalid_argument("missing action")),
		};

		global.nats().publish(reply, ack_kind.into()).await.map_err(|err| {
			tracing::error!(err = %err, "failed to publish ack");
			tonic::Status::internal("failed to publish ack")
		})?;

		let lease_duration = config.nats_stream_message_lease_duration.as_secs().max(1) as i64;

		if matches!(ack_kind, AckKind::Progress) {
			global.redis().expire(&key, lease_duration)
		} else {
			global.redis().del(&key)
		}
		.await
		.map_err(|err| {
			tracing::error!(err = %err, "failed to delete event id from redis");
			tonic::Status::internal("failed to delete event id from redis")
		})?;

		Ok(tonic::Response::new(EventsAckResponse {}))
	}
}
