use std::sync::Arc;

use pb::ext::UlidExt;
use pb::scuffle::video::v1::events_fetch_request::Target;
use pb::scuffle::video::v1::types::access_token_scope::Permission;
use pb::scuffle::video::v1::types::{event, Resource};
use pb::scuffle::video::v1::{PlaybackKeyPairModifyRequest, PlaybackKeyPairModifyResponse};
use tonic::Status;
use video_common::database::{AccessToken, DatabaseTable};

use super::utils::validate_public_key;
use crate::api::errors::MODIFY_NO_FIELDS;
use crate::api::utils::tags::validate_tags;
use crate::api::utils::{impl_request_scopes, ApiRequest, TonicRequest};
use crate::global::ApiGlobal;
use crate::ratelimit::RateLimitResource;

impl_request_scopes!(
	PlaybackKeyPairModifyRequest,
	video_common::database::PlaybackKeyPair,
	(Resource::PlaybackKeyPair, Permission::Modify),
	RateLimitResource::PlaybackKeyPairModify
);

pub fn validate(req: &PlaybackKeyPairModifyRequest) -> tonic::Result<()> {
	validate_tags(req.tags.as_ref())
}

pub fn build_query(
	req: &PlaybackKeyPairModifyRequest,
	access_token: &AccessToken,
) -> tonic::Result<utils::database::QueryBuilder<'static>> {
	let mut qb = scuffle_utils::database::QueryBuilder::default();

	qb.push("UPDATE ")
		.push(<PlaybackKeyPairModifyRequest as TonicRequest>::Table::NAME)
		.push(" SET ");

	let mut seperated = qb.separated(",");

	if let Some(tags) = &req.tags {
		seperated
			.push("tags = ")
			.push_bind_unseparated(utils::database::Json(tags.tags.clone()));
	}

	if let Some(public_key) = &req.public_key {
		let (cert, fingerprint) = validate_public_key(public_key)?;

		seperated.push("public_key = ").push_bind_unseparated(cert.into_bytes());
		seperated.push("fingerprint = ").push_bind_unseparated(fingerprint);
	}

	if req.tags.is_none() && req.public_key.is_none() {
		return Err(tonic::Status::invalid_argument(MODIFY_NO_FIELDS));
	}

	seperated.push("updated_at = ").push_bind_unseparated(chrono::Utc::now());

	qb.push(" WHERE id = ").push_bind(req.id.into_ulid());
	qb.push(" AND organization_id = ").push_bind(access_token.organization_id);
	qb.push(" RETURNING *");

	Ok(qb)
}

impl ApiRequest<PlaybackKeyPairModifyResponse> for tonic::Request<PlaybackKeyPairModifyRequest> {
	async fn process<G: ApiGlobal>(
		&self,
		global: &Arc<G>,
		access_token: &AccessToken,
	) -> tonic::Result<tonic::Response<PlaybackKeyPairModifyResponse>> {
		let req = self.get_ref();

		validate(req)?;

		let query = build_query(req, access_token)?;

		let result: Option<video_common::database::PlaybackKeyPair> =
			query.build_query_as().fetch_optional(global.db()).await.map_err(|err| {
				tracing::error!(err = %err, "failed to modify {}", <PlaybackKeyPairModifyRequest as TonicRequest>::Table::FRIENDLY_NAME);
				Status::internal(format!(
					"failed to modify {}",
					<PlaybackKeyPairModifyRequest as TonicRequest>::Table::FRIENDLY_NAME
				))
			})?;

		match result {
			Some(result) => {
				video_common::events::emit(
					global.nats(),
					&global.config().events.stream_name,
					access_token.organization_id,
					Target::PlaybackKeyPair,
					event::Event::PlaybackKeyPair(event::PlaybackKeyPair {
						playback_key_pair_id: Some(result.id.into()),
						event: Some(event::playback_key_pair::Event::Modified(
							event::playback_key_pair::Modified {},
						)),
					}),
				)
				.await;
				Ok(tonic::Response::new(PlaybackKeyPairModifyResponse {
					playback_key_pair: Some(result.into_proto()),
				}))
			}
			None => Err(tonic::Status::not_found(format!(
				"{} not found",
				<PlaybackKeyPairModifyRequest as TonicRequest>::Table::FRIENDLY_NAME
			))),
		}
	}
}
