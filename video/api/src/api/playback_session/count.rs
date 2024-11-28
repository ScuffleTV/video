use std::sync::Arc;

use pb::scuffle::video::v1::types::access_token_scope::Permission;
use pb::scuffle::video::v1::types::{playback_session_target, Resource};
use pb::scuffle::video::v1::{playback_session_count_request, PlaybackSessionCountRequest, PlaybackSessionCountResponse};
use video_common::database::{AccessToken, DatabaseTable};

use crate::api::utils::{impl_request_scopes, ApiRequest, TonicRequest};
use crate::global::ApiGlobal;
use crate::ratelimit::RateLimitResource;

impl_request_scopes!(
	PlaybackSessionCountRequest,
	video_common::database::PlaybackSession,
	(Resource::PlaybackSession, Permission::Read),
	RateLimitResource::PlaybackSessionCount
);

#[derive(postgres_from_row::FromRow)]
pub struct PlaybackSessionCountQueryResp {
	total_count: i64,
	deduped: i64,
}

pub fn build_query<'a>(
	req: &'a PlaybackSessionCountRequest,
	access_token: &AccessToken,
) -> tonic::Result<utils::database::QueryBuilder<'a>> {
	let mut qb = scuffle_utils::database::QueryBuilder::default();

	let filter = req
		.filter
		.as_ref()
		.ok_or_else(|| tonic::Status::invalid_argument("filter is required"))?;

	qb.push("SELECT COUNT(*) AS total_count, COUNT(DISTINCT ");

	match filter {
		playback_session_count_request::Filter::UserId(user_id) => {
			qb.push("(recording_id, room_id)) AS deduped FROM playback_sessions WHERE user_id = ")
				.push_bind(user_id)
				.push(" AND organization_id = ")
				.push_bind(access_token.organization_id);
		}
		playback_session_count_request::Filter::Target(target) => {
			let target = target
				.target
				.ok_or_else(|| tonic::Status::invalid_argument("filter is required"))?;

			qb.push("COALESCE(user_id, ip_address::text)) AS deduped FROM playback_sessions WHERE organization_id = ")
				.push_bind(access_token.organization_id);
			qb.push(" AND ");

			match target {
				playback_session_target::Target::RecordingId(recording_id) => {
					qb.push("recording_id = ").push_bind(recording_id.into_ulid());
					qb.push(" AND room_id IS NULL");
				}
				playback_session_target::Target::RoomId(room_id) => {
					qb.push("room_id = ").push_bind(room_id.into_ulid());
					qb.push(" AND recording_id IS NULL");
				}
			}
		}
	}

	Ok(qb)
}

impl ApiRequest<PlaybackSessionCountResponse> for tonic::Request<PlaybackSessionCountRequest> {
	async fn process<G: ApiGlobal>(
		&self,
		global: &Arc<G>,
		access_token: &AccessToken,
	) -> tonic::Result<tonic::Response<PlaybackSessionCountResponse>> {
		let req = self.get_ref();

		let query = build_query(req, access_token)?;

		let result: Option<PlaybackSessionCountQueryResp> =
			query.build_query_as().fetch_optional(global.db()).await.map_err(|err| {
				tracing::error!(err = %err, "failed to fetch {}s", <PlaybackSessionCountRequest as TonicRequest>::Table::FRIENDLY_NAME);
				tonic::Status::internal(format!(
					"failed to fetch {}s",
					<PlaybackSessionCountRequest as TonicRequest>::Table::FRIENDLY_NAME
				))
			})?;

		match result {
			Some(result) => Ok(tonic::Response::new(PlaybackSessionCountResponse {
				count: result.total_count as u64,
				deduplicated_count: result.deduped as u64,
			})),
			None => Ok(tonic::Response::new(PlaybackSessionCountResponse {
				count: 0,
				deduplicated_count: 0,
			})),
		}
	}
}
