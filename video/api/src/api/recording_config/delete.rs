use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use pb::scuffle::video::v1::events_fetch_request::Target;
use pb::scuffle::video::v1::types::access_token_scope::Permission;
use pb::scuffle::video::v1::types::{event, FailedResource, Resource};
use pb::scuffle::video::v1::{RecordingConfigDeleteRequest, RecordingConfigDeleteResponse};
use tonic::Status;
use ulid::Ulid;
use video_common::database::{AccessToken, DatabaseTable};

use crate::api::utils::{impl_request_scopes, ApiRequest, TonicRequest};
use crate::global::ApiGlobal;
use crate::ratelimit::RateLimitResource;

impl_request_scopes!(
	RecordingConfigDeleteRequest,
	video_common::database::RecordingConfig,
	(Resource::RecordingConfig, Permission::Delete),
	RateLimitResource::RecordingConfigDelete
);

impl ApiRequest<RecordingConfigDeleteResponse> for tonic::Request<RecordingConfigDeleteRequest> {
	async fn process<G: ApiGlobal>(
		&self,
		global: &Arc<G>,
		access_token: &AccessToken,
	) -> tonic::Result<tonic::Response<RecordingConfigDeleteResponse>> {
		// Check if any rooms are using the recording config
		let mut qb = scuffle_utils::database::QueryBuilder::default();

		let req = self.get_ref();

		if req.ids.len() > 100 {
			return Err(tonic::Status::invalid_argument(
				"too many ids provided for delete: max 100".to_string(),
			));
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

		qb.push("(SELECT DISTINCT recording_config_id AS id FROM ")
			.push(<video_common::database::Room as DatabaseTable>::NAME)
			.push(" WHERE recording_config_id = ANY(")
			.push_bind(ids_to_delete.iter().copied().collect::<Vec<_>>())
			.push(") AND organization_id = ")
			.push_bind(access_token.organization_id)
			.push(") UNION (SELECT DISTINCT recording_config_id AS id FROM ")
			.push(<video_common::database::Recording as DatabaseTable>::NAME)
			.push(" WHERE recording_config_id = ANY($1) AND organization_id = $2)");

		let client = global.db().get().await.map_err(|err| {
			tracing::error!(err = %err, "failed to get db client");
			Status::internal("internal server error")
		})?;

		let used_configs: Vec<Ulid> = qb.build_query_single_scalar().fetch_all(&client).await.map_err(|err| {
			tracing::error!(err = %err, "failed to check if any {}s are being used", <RecordingConfigDeleteRequest as TonicRequest>::Table::FRIENDLY_NAME);
			Status::internal(format!("failed to check if any {}s are being used", <RecordingConfigDeleteRequest as TonicRequest>::Table::FRIENDLY_NAME))
		})?;

		let mut failed_deletes = used_configs
			.into_iter()
			.map(|id| {
				ids_to_delete.remove(&id);
				(id, "recording config in use")
			})
			.collect::<HashMap<_, _>>();

		let deleted_ids = if !ids_to_delete.is_empty() {
			// Delete the recording config
			let mut qb = scuffle_utils::database::QueryBuilder::default();

			qb.push("DELETE FROM ")
				.push(<RecordingConfigDeleteRequest as TonicRequest>::Table::NAME)
				.push(" WHERE id = ANY(")
				.push_bind(ids_to_delete.iter().copied().collect::<Vec<_>>())
				.push(") AND organization_id = ")
				.push_bind(access_token.organization_id)
				.push(" RETURNING id");

			let deleted_ids: Vec<Ulid> = qb.build_query_single_scalar().fetch_all(&client).await.map_err(|err| {
				tracing::error!(err = %err, "failed to delete {}", <RecordingConfigDeleteRequest as TonicRequest>::Table::FRIENDLY_NAME);
				Status::internal(format!(
					"failed to delete {}",
					<RecordingConfigDeleteRequest as TonicRequest>::Table::FRIENDLY_NAME
				))
			})?;

			deleted_ids.iter().for_each(|id| {
				ids_to_delete.remove(id);
			});

			deleted_ids
		} else {
			Default::default()
		};

		drop(client);

		for id in deleted_ids.iter().copied() {
			video_common::events::emit(
				global.nats(),
				&global.config().events.stream_name,
				access_token.organization_id,
				Target::RecordingConfig,
				event::Event::RecordingConfig(event::RecordingConfig {
					recording_config_id: Some(id.into()),
					event: Some(event::recording_config::Event::Deleted(event::recording_config::Deleted {})),
				}),
			)
			.await;
		}

		ids_to_delete.into_iter().for_each(|id| {
			failed_deletes.insert(id, "recording config not found");
		});

		Ok(tonic::Response::new(RecordingConfigDeleteResponse {
			ids: deleted_ids.into_iter().map(|id| id.into()).collect(),
			failed_deletes: failed_deletes
				.into_iter()
				.map(|(id, reason)| FailedResource {
					id: Some(id.into()),
					reason: reason.to_string(),
				})
				.collect(),
		}))
	}
}
