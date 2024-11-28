use std::sync::Arc;

use pb::scuffle::video::v1::events_fetch_request::Target;
use pb::scuffle::video::v1::types::access_token_scope::Permission;
use pb::scuffle::video::v1::types::{event, Resource};
use pb::scuffle::video::v1::{PlaybackKeyPairCreateRequest, PlaybackKeyPairCreateResponse};
use tonic::Status;
use ulid::Ulid;
use video_common::database::{AccessToken, DatabaseTable};

use super::utils::validate_public_key;
use crate::api::utils::tags::validate_tags;
use crate::api::utils::{impl_request_scopes, ApiRequest, TonicRequest};
use crate::global::ApiGlobal;
use crate::ratelimit::RateLimitResource;

impl_request_scopes!(
	PlaybackKeyPairCreateRequest,
	video_common::database::PlaybackKeyPair,
	(Resource::PlaybackKeyPair, Permission::Create),
	RateLimitResource::PlaybackKeyPairCreate
);

pub fn validate(req: &PlaybackKeyPairCreateRequest) -> tonic::Result<(String, String)> {
	validate_tags(req.tags.as_ref())?;
	validate_public_key(&req.public_key)
}

pub fn build_query(
	req: &PlaybackKeyPairCreateRequest,
	access_token: &AccessToken,
	jwt: (String, String),
) -> tonic::Result<utils::database::QueryBuilder<'static>> {
	let (cert, fingerprint) = jwt;

	let mut qb = scuffle_utils::database::QueryBuilder::default();

	qb.push("INSERT INTO ")
		.push(<PlaybackKeyPairCreateRequest as TonicRequest>::Table::NAME)
		.push(" (");

	let mut seperated = qb.separated(",");

	seperated.push("id");
	seperated.push("organization_id");
	seperated.push("public_key");
	seperated.push("fingerprint");
	seperated.push("updated_at");
	seperated.push("tags");

	qb.push(") VALUES (");

	let mut seperated = qb.separated(",");

	seperated.push_bind(Ulid::new());
	seperated.push_bind(access_token.organization_id);
	seperated.push_bind(cert.into_bytes());
	seperated.push_bind(fingerprint);
	seperated.push_bind(chrono::Utc::now());
	seperated.push_bind(utils::database::Json(req.tags.clone().unwrap_or_default().tags));

	qb.push(") RETURNING *");

	Ok(qb)
}

impl ApiRequest<PlaybackKeyPairCreateResponse> for tonic::Request<PlaybackKeyPairCreateRequest> {
	async fn process<G: ApiGlobal>(
		&self,
		global: &Arc<G>,
		access_token: &AccessToken,
	) -> tonic::Result<tonic::Response<PlaybackKeyPairCreateResponse>> {
		let req = self.get_ref();

		let jwt = validate(req)?;

		let query = build_query(req, access_token, jwt)?;

		let playback_key_pair: video_common::database::PlaybackKeyPair =
			query.build_query_as().fetch_one(global.db()).await.map_err(|err| {
				tracing::error!(err = %err, "failed to create {}", <PlaybackKeyPairCreateRequest as TonicRequest>::Table::FRIENDLY_NAME);
				Status::internal(format!(
					"failed to create {}",
					<PlaybackKeyPairCreateRequest as TonicRequest>::Table::FRIENDLY_NAME
				))
			})?;

		video_common::events::emit(
			global.nats(),
			&global.config().events.stream_name,
			access_token.organization_id,
			Target::PlaybackKeyPair,
			event::Event::PlaybackKeyPair(event::PlaybackKeyPair {
				playback_key_pair_id: Some(playback_key_pair.id.into()),
				event: Some(event::playback_key_pair::Event::Created(event::playback_key_pair::Created {})),
			}),
		)
		.await;

		Ok(tonic::Response::new(PlaybackKeyPairCreateResponse {
			playback_key_pair: Some(playback_key_pair.into_proto()),
		}))
	}
}
