use std::sync::Arc;

use pb::ext::UlidExt;
use pb::scuffle::video::v1::events_fetch_request::Target;
use pb::scuffle::video::v1::types::access_token_scope::Permission;
use pb::scuffle::video::v1::types::{event, Resource};
use pb::scuffle::video::v1::{RecordingModifyRequest, RecordingModifyResponse};
use tonic::Status;
use video_common::database::{AccessToken, DatabaseTable, Visibility};

use crate::api::errors::MODIFY_NO_FIELDS;
use crate::api::utils::tags::validate_tags;
use crate::api::utils::{impl_request_scopes, ApiRequest, TonicRequest};
use crate::global::ApiGlobal;
use crate::ratelimit::RateLimitResource;

impl_request_scopes!(
	RecordingModifyRequest,
	video_common::database::Recording,
	(Resource::Recording, Permission::Modify),
	RateLimitResource::RecordingModify
);

impl ApiRequest<RecordingModifyResponse> for tonic::Request<RecordingModifyRequest> {
	async fn process<G: ApiGlobal>(
		&self,
		global: &Arc<G>,
		access_token: &AccessToken,
	) -> tonic::Result<tonic::Response<RecordingModifyResponse>> {
		let req = self.get_ref();

		validate_tags(req.tags.as_ref())?;

		let mut qb = scuffle_utils::database::QueryBuilder::default();

		qb.push("UPDATE ")
			.push(<RecordingModifyRequest as TonicRequest>::Table::NAME)
			.push(" SET ");

		let mut seperated = qb.separated(", ");

		let client = global.db().get().await.map_err(|err| {
			tracing::error!(err = %err, "failed to get db client");
			Status::internal("internal server error")
		})?;

		if let Some(room_id) = &req.room_id {
			scuffle_utils::database::query("SELECT id FROM rooms WHERE id = $1 AND organization_id = $2")
				.bind(room_id.into_ulid())
				.bind(access_token.organization_id)
				.build()
				.fetch_optional(&client)
				.await
				.map_err(|err| {
					tracing::error!(err = %err, "failed to query room");
					Status::internal("failed to query rooms")
				})?
				.ok_or_else(|| Status::not_found("room not found"))?;

			seperated.push("room_id = ").push_bind_unseparated(room_id.into_ulid());
		}

		if let Some(recording_config_id) = &req.recording_config_id {
			scuffle_utils::database::query("SELECT id FROM recording_configs WHERE id = $1 AND organization_id = $2")
				.bind(recording_config_id.into_ulid())
				.bind(access_token.organization_id)
				.build()
				.fetch_optional(&client)
				.await
				.map_err(|err| {
					tracing::error!(err = %err, "failed to query recording config");
					Status::internal("failed to query recording configs")
				})?
				.ok_or_else(|| Status::not_found("recording config not found"))?;

			seperated
				.push("recording_config_id = ")
				.push_bind_unseparated(recording_config_id.into_ulid());
		}

		if let Some(visibility) = req.visibility {
			let visibility = pb::scuffle::video::v1::types::Visibility::try_from(visibility)
				.map_err(|_| Status::invalid_argument("invalid visibility value"))?;

			seperated
				.push("visibility = ")
				.push_bind_unseparated(Visibility::from(visibility));
		}

		if let Some(tags) = &req.tags {
			seperated
				.push("tags = ")
				.push_bind_unseparated(utils::database::Json(&tags.tags));
		}

		if req.tags.is_none() && req.room_id.is_none() && req.recording_config_id.is_none() && req.visibility.is_none() {
			return Err(Status::invalid_argument(MODIFY_NO_FIELDS));
		}

		seperated.push("updated_at = ").push_bind_unseparated(chrono::Utc::now());

		qb.push(" WHERE id = ").push_bind(req.id.into_ulid());
		qb.push(" AND organization_id = ").push_bind(access_token.organization_id);
		qb.push(" RETURNING *");

		let result = qb
			.build_query_as::<<RecordingModifyRequest as TonicRequest>::Table>()
			.fetch_optional(client)
			.await
			.map_err(|err| {
				tracing::error!(err = %err, "failed to update recording");
				Status::internal("failed to update recording")
			})?
			.ok_or_else(|| Status::not_found("recording not found"))?;

		let state = global
			.recording_state_loader()
			.load((result.organization_id, result.id))
			.await
			.map_err(|_| Status::internal("failed to load recording state"))?
			.unwrap_or_default();

		video_common::events::emit(
			global.nats(),
			&global.config().events.stream_name,
			access_token.organization_id,
			Target::Recording,
			event::Event::Recording(event::Recording {
				recording_id: Some(result.id.into()),
				event: Some(event::recording::Event::Modified(event::recording::Modified {})),
			}),
		)
		.await;

		Ok(tonic::Response::new(RecordingModifyResponse {
			recording: Some(state.recording_to_proto(result)),
		}))
	}
}
