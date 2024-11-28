use std::collections::HashSet;
use std::sync::Arc;

use pb::scuffle::video::v1::events_fetch_request::Target;
use pb::scuffle::video::v1::room_reset_key_response::RoomKeyPair;
use pb::scuffle::video::v1::types::access_token_scope::Permission;
use pb::scuffle::video::v1::types::{event, FailedResource, Resource};
use pb::scuffle::video::v1::{room_reset_key_response, RoomResetKeyRequest, RoomResetKeyResponse};
use ulid::Ulid;
use video_common::database::{AccessToken, DatabaseTable};

use super::utils::create_stream_key;
use crate::api::utils::{impl_request_scopes, ApiRequest, TonicRequest};
use crate::global::ApiGlobal;
use crate::ratelimit::RateLimitResource;

impl_request_scopes!(
	RoomResetKeyRequest,
	video_common::database::Room,
	(Resource::Room, Permission::Modify),
	RateLimitResource::RoomResetKey
);

#[derive(postgres_from_row::FromRow)]
struct RoomResetKeyRow {
	id: Ulid,
	stream_key: String,
}

impl ApiRequest<RoomResetKeyResponse> for tonic::Request<RoomResetKeyRequest> {
	async fn process<G: ApiGlobal>(
		&self,
		global: &Arc<G>,
		access_token: &AccessToken,
	) -> tonic::Result<tonic::Response<RoomResetKeyResponse>> {
		let req = self.get_ref();

		if req.ids.len() > 100 {
			return Err(tonic::Status::invalid_argument("too many ids provided for delete: max 100"));
		}

		if req.ids.is_empty() {
			return Err(tonic::Status::invalid_argument("no ids provided for delete"));
		}

		let mut ids_to_reset = req
			.ids
			.iter()
			.copied()
			.map(pb::scuffle::types::Ulid::into_ulid)
			.collect::<HashSet<_>>();

		let data = ids_to_reset.iter().copied().map(|id| (id, create_stream_key()));

		let mut qb = scuffle_utils::database::QueryBuilder::default();

		qb.push("WITH updated_values AS (SELECT * FROM (")
			.push_values(data.clone(), |mut b, data| {
				b.push_bind(data.0)
					.push_unseparated("::UUID")
					.push_bind(data.1)
					.push_unseparated("::TEXT");
			})
			.push(") AS v (id, stream_key)) UPDATE ")
			.push(<RoomResetKeyRequest as TonicRequest>::Table::NAME)
			.push(" r SET stream_key = uv.stream_key FROM updated_values uv WHERE r.id = uv.id AND r.organization_id = ")
			.push_bind(access_token.organization_id)
			.push(" RETURNING r.id, r.stream_key");

		let rows: Vec<RoomResetKeyRow> = qb.build_query_as().fetch_all(global.db()).await.map_err(|err| {
			tracing::error!(err = %err, "failed to reset room stream keys");
			tonic::Status::internal("failed to reset room stream keys")
		})?;

		let rooms = rows
			.into_iter()
			.map(|row| {
				ids_to_reset.remove(&row.id);

				room_reset_key_response::RoomKeyPair {
					id: Some(row.id.into()),
					key: row.stream_key,
				}
			})
			.collect::<Vec<_>>();

		let failed_resets = ids_to_reset
			.into_iter()
			.map(|id| FailedResource {
				id: Some(id.into()),
				reason: "room not found".into(),
			})
			.collect::<Vec<_>>();

		for RoomKeyPair { id, .. } in rooms.iter() {
			if let Some(id) = id {
				video_common::events::emit(
					global.nats(),
					&global.config().events.stream_name,
					access_token.organization_id,
					Target::Room,
					event::Event::Room(event::Room {
						room_id: Some(*id),
						event: Some(event::room::Event::Modified(event::room::Modified {})),
					}),
				)
				.await;
			}
		}

		Ok(tonic::Response::new(RoomResetKeyResponse { rooms, failed_resets }))
	}
}
