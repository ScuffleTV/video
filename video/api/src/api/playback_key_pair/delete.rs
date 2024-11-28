use std::collections::HashSet;
use std::sync::Arc;

use pb::scuffle::video::v1::events_fetch_request::Target;
use pb::scuffle::video::v1::types::access_token_scope::Permission;
use pb::scuffle::video::v1::types::{event, FailedResource, Resource};
use pb::scuffle::video::v1::{PlaybackKeyPairDeleteRequest, PlaybackKeyPairDeleteResponse};
use tonic::Status;
use ulid::Ulid;
use video_common::database::{AccessToken, DatabaseTable};

use crate::api::utils::{impl_request_scopes, ApiRequest, TonicRequest};
use crate::global::ApiGlobal;
use crate::ratelimit::RateLimitResource;

impl_request_scopes!(
	PlaybackKeyPairDeleteRequest,
	video_common::database::PlaybackKeyPair,
	(Resource::PlaybackKeyPair, Permission::Delete),
	RateLimitResource::PlaybackKeyPairDelete
);

impl ApiRequest<PlaybackKeyPairDeleteResponse> for tonic::Request<PlaybackKeyPairDeleteRequest> {
	async fn process<G: ApiGlobal>(
		&self,
		global: &Arc<G>,
		access_token: &AccessToken,
	) -> tonic::Result<tonic::Response<PlaybackKeyPairDeleteResponse>> {
		let req = self.get_ref();

		if req.ids.len() > 100 {
			return Err(tonic::Status::invalid_argument("too many ids provided for delete: max 100"));
		}

		if req.ids.is_empty() {
			return Err(tonic::Status::invalid_argument("no ids provided for delete"));
		}

		let mut ids_to_delete = req
			.ids
			.iter()
			.copied()
			.map(pb::scuffle::types::Ulid::into_ulid)
			.collect::<HashSet<_>>();

		let deleted_ids: Vec<Ulid> = scuffle_utils::database::query("DELETE FROM ")
			.push(<PlaybackKeyPairDeleteRequest as TonicRequest>::Table::NAME)
			.push(" WHERE id = ANY(")
			.push_bind(ids_to_delete.iter().copied().collect::<Vec<_>>())
			.push(") AND organization_id = ")
			.push_bind(access_token.organization_id)
			.push(" RETURNING id")
			.build_query_single_scalar()
			.fetch_all(global.db())
			.await
			.map_err(|err| {
				tracing::error!(err = %err, "failed to delete {}", <PlaybackKeyPairDeleteRequest as TonicRequest>::Table::FRIENDLY_NAME);
				Status::internal(format!(
					"failed to delete {}",
					<PlaybackKeyPairDeleteRequest as TonicRequest>::Table::FRIENDLY_NAME
				))
			})?;

		for id in deleted_ids.iter().copied() {
			video_common::events::emit(
				global.nats(),
				&global.config().events.stream_name,
				access_token.organization_id,
				Target::PlaybackKeyPair,
				event::Event::PlaybackKeyPair(event::PlaybackKeyPair {
					playback_key_pair_id: Some(id.into()),
					event: Some(event::playback_key_pair::Event::Deleted(event::playback_key_pair::Deleted {})),
				}),
			)
			.await;
		}

		deleted_ids.iter().for_each(|id| {
			ids_to_delete.remove(id);
		});

		Ok(tonic::Response::new(PlaybackKeyPairDeleteResponse {
			ids: deleted_ids.into_iter().map(|id| id.into()).collect(),
			failed_deletes: ids_to_delete
				.into_iter()
				.map(|id| FailedResource {
					id: Some(id.into()),
					reason: "playback key pair not found".into(),
				})
				.collect(),
		}))
	}
}
