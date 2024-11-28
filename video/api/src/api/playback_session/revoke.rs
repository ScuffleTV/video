use std::collections::HashSet;
use std::sync::Arc;

use chrono::TimeZone;
use pb::scuffle::video::v1::types::access_token_scope::Permission;
use pb::scuffle::video::v1::types::{playback_session_target, Resource};
use pb::scuffle::video::v1::{PlaybackSessionRevokeRequest, PlaybackSessionRevokeResponse};
use ulid::Ulid;
use video_common::database::AccessToken;

use crate::api::utils::{impl_request_scopes, ApiRequest};
use crate::global::ApiGlobal;
use crate::ratelimit::RateLimitResource;

impl_request_scopes!(
	PlaybackSessionRevokeRequest,
	video_common::database::PlaybackSession,
	(Resource::PlaybackSession, Permission::Delete),
	RateLimitResource::PlaybackSessionRevoke
);

impl ApiRequest<PlaybackSessionRevokeResponse> for tonic::Request<PlaybackSessionRevokeRequest> {
	async fn process<G: ApiGlobal>(
		&self,
		global: &Arc<G>,
		access_token: &AccessToken,
	) -> tonic::Result<tonic::Response<PlaybackSessionRevokeResponse>> {
		let mut qb = scuffle_utils::database::QueryBuilder::default();

		let req = self.get_ref();

		if req.ids.len() > 100 {
			return Err(tonic::Status::invalid_argument("too many ids provided for revoke: max 100"));
		}

		qb.push("DELETE FROM playback_sessions WHERE ");

		let mut seperated = qb.separated(" AND ");

		seperated
			.push("organization_id = ")
			.push_bind_unseparated(access_token.organization_id);

		if let Some(before) = req.before {
			if before < 0 {
				return Err(tonic::Status::invalid_argument("before must be positive"));
			}

			seperated
				.push("id < ")
				.push_bind_unseparated(Ulid::from_parts(before as u64, 0));
		}

		if !req.ids.is_empty() {
			let ids = req
				.ids
				.iter()
				.copied()
				.map(pb::scuffle::types::Ulid::into_ulid)
				.collect::<HashSet<_>>();

			seperated
				.push("id = ANY(")
				.push_bind_unseparated(ids.into_iter().collect::<Vec<_>>())
				.push_unseparated(")");
		}

		if let Some(user_id) = &req.user_id {
			seperated.push("user_id = ").push_bind_unseparated(user_id);
		}

		if let Some(authorized) = req.authorized {
			if req.user_id.is_some() {
				return Err(tonic::Status::invalid_argument(
					"cannot specify both user_id and unauthorized",
				));
			}

			if authorized {
				seperated.push("user_id IS NOT NULL");
			} else {
				seperated.push("user_id IS NULL");
			}
		}

		match req.target.and_then(|t| t.target) {
			Some(playback_session_target::Target::RecordingId(recording_id)) => {
				seperated
					.push("recording_id = ")
					.push_bind_unseparated(recording_id.into_ulid());
			}
			Some(playback_session_target::Target::RoomId(room_id)) => {
				seperated.push("room_id = ").push_bind_unseparated(room_id.into_ulid());
			}
			None => {}
		}

		let mut client = global.db().get().await.map_err(|err| {
			tracing::error!(err = %err, "failed to get db client");
			tonic::Status::internal("internal server error")
		})?;

		let tx = client.transaction().await.map_err(|e| {
			tracing::error!(err = %e, "beginning transaction");
			tonic::Status::internal("playback session revoke failed")
		})?;

		let result = qb.build().execute(&tx).await.map_err(|e| {
			tracing::error!(err = %e, "revoking playback sessions");
			tonic::Status::internal("playback session revoke failed")
		})?;

		if req.ids.is_empty()
			&& req.before.map_or(true, |b| {
				chrono::Utc.timestamp_millis_opt(b).unwrap() > chrono::Utc::now() - chrono::Duration::minutes(10)
			}) {
			scuffle_utils::database::query("INSERT INTO playback_session_revocations(organization_id, room_id, recording_id, user_id, revoke_before) VALUES ($1, $2, $3, $4, $5)")
			.bind(access_token.organization_id)
			.bind(req.target.and_then(|t| match t.target {
					Some(playback_session_target::Target::RoomId(room_id)) => Some(room_id.into_ulid()),
					_ => None,
			}))
			.bind(req.target.and_then(|t| match t.target {
					Some(playback_session_target::Target::RecordingId(recording_id)) => Some(recording_id.into_ulid()),
					_ => None,
				}))
			.bind(&req.user_id)
			.bind(req.before.map_or_else(chrono::Utc::now, |b| chrono::Utc.timestamp_millis_opt(b).unwrap()))
			.build()
			.execute(&tx)
			.await.map_err(|e| {
				tracing::error!(err = %e, "playback session revoke failed");
				tonic::Status::internal("playback session revoke failed")
			})?;
		}

		tx.commit().await.map_err(|e| {
			tracing::error!(err = %e, "committing transaction");
			tonic::Status::internal("playback session revoke failed")
		})?;

		Ok(tonic::Response::new(PlaybackSessionRevokeResponse { revoked: result }))
	}
}
