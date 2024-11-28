use std::sync::Arc;

use pb::scuffle::video::v1::types::access_token_scope::Permission;
use pb::scuffle::video::v1::types::{playback_session_target, Resource};
use pb::scuffle::video::v1::{PlaybackSessionGetRequest, PlaybackSessionGetResponse};
use video_common::database::{AccessToken, DatabaseTable};

use crate::api::utils::{get, impl_request_scopes, ApiRequest, TonicRequest};
use crate::global::ApiGlobal;
use crate::ratelimit::RateLimitResource;

impl_request_scopes!(
	PlaybackSessionGetRequest,
	video_common::database::PlaybackSession,
	(Resource::PlaybackSession, Permission::Read),
	RateLimitResource::PlaybackSessionGet
);

pub fn build_query<'a>(
	req: &'a PlaybackSessionGetRequest,
	access_token: &AccessToken,
) -> tonic::Result<utils::database::QueryBuilder<'a>> {
	let mut qb = scuffle_utils::database::QueryBuilder::default();
	qb.push("SELECT * FROM ")
		.push(<PlaybackSessionGetRequest as TonicRequest>::Table::NAME)
		.push(" WHERE ");
	let mut seperated = qb.separated(" AND ");

	get::organization_id(&mut seperated, access_token.organization_id);
	get::ids(&mut seperated, &req.ids);

	if let Some(user_id) = req.user_id.as_ref() {
		seperated.push("user_id = ");
		seperated.push_bind_unseparated(user_id);
	}

	if let Some(playback_key_pair_id) = req.playback_key_pair_id {
		seperated.push("playback_key_pair_id = ");
		seperated.push_bind_unseparated(playback_key_pair_id.into_ulid());
	} else if let Some(authorized) = req.authorized {
		if authorized {
			seperated.push("playback_key_pair_id IS NOT NULL");
		} else {
			seperated.push("playback_key_pair_id IS NULL");
		}
	}

	if let Some(ip_address) = req.ip_address.as_ref() {
		let ip = ip_address
			.parse::<std::net::IpAddr>()
			.map_err(|_| tonic::Status::invalid_argument(format!("invalid ip address: {}", ip_address)))?;

		seperated.push("ip_address = ");
		seperated.push_bind_unseparated(ip);
	}

	if let Some(target) = req.target {
		match target.target {
			Some(playback_session_target::Target::RecordingId(recording_id)) => {
				seperated.push("recording_id = ");
				seperated.push_bind_unseparated(recording_id.into_ulid());
			}
			Some(playback_session_target::Target::RoomId(room_id)) => {
				seperated.push("room_id = ");
				seperated.push_bind_unseparated(room_id.into_ulid());
			}
			None => {}
		}
	}

	if let Some(tags) = req.search_options.as_ref().and_then(|o| o.tags.as_ref()) {
		if !tags.tags.is_empty() {
			return Err(tonic::Status::invalid_argument("tags are not supported by playback sessions"));
		}
	}

	get::search_options(&mut seperated, req.search_options.as_ref())?;

	Ok(qb)
}

impl ApiRequest<PlaybackSessionGetResponse> for tonic::Request<PlaybackSessionGetRequest> {
	async fn process<G: ApiGlobal>(
		&self,
		global: &Arc<G>,
		access_token: &AccessToken,
	) -> tonic::Result<tonic::Response<PlaybackSessionGetResponse>> {
		let req = self.get_ref();

		let query = build_query(req, access_token)?;

		let results = query.build_query_as().fetch_all(global.db()).await.map_err(|err| {
			tracing::error!(err = %err, "failed to fetch {}s", <PlaybackSessionGetRequest as TonicRequest>::Table::FRIENDLY_NAME);
			tonic::Status::internal(format!(
				"failed to fetch {}s",
				<PlaybackSessionGetRequest as TonicRequest>::Table::FRIENDLY_NAME
			))
		})?;

		Ok(tonic::Response::new(PlaybackSessionGetResponse {
			sessions: results
				.into_iter()
				.map(video_common::database::PlaybackSession::into_proto)
				.collect(),
		}))
	}
}
