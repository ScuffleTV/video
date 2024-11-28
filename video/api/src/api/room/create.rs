use std::sync::Arc;

use pb::scuffle::video::v1::events_fetch_request::Target;
use pb::scuffle::video::v1::types::access_token_scope::Permission;
use pb::scuffle::video::v1::types::{event, Resource};
use pb::scuffle::video::v1::{RoomCreateRequest, RoomCreateResponse};
use scuffle_utils::database::ClientLike;
use tonic::Status;
use ulid::Ulid;
use video_common::database::{AccessToken, DatabaseTable, Visibility};

use super::utils::create_stream_key;
use crate::api::utils::tags::validate_tags;
use crate::api::utils::{impl_request_scopes, ApiRequest, TonicRequest};
use crate::global::ApiGlobal;
use crate::ratelimit::RateLimitResource;

impl_request_scopes!(
	RoomCreateRequest,
	video_common::database::Room,
	(Resource::Room, Permission::Create),
	RateLimitResource::RoomCreate
);

pub fn validate(req: &RoomCreateRequest) -> tonic::Result<()> {
	validate_tags(req.tags.as_ref())
}

pub async fn build_query(
	req: &RoomCreateRequest,
	client: impl ClientLike,
	access_token: &AccessToken,
) -> tonic::Result<utils::database::QueryBuilder<'static>> {
	let mut qb = scuffle_utils::database::QueryBuilder::default();

	qb.push("INSERT INTO ")
		.push(<RoomCreateRequest as TonicRequest>::Table::NAME)
		.push(" (");

	let mut seperated = qb.separated(",");

	seperated.push("id");
	seperated.push("organization_id");
	seperated.push("transcoding_config_id");
	seperated.push("recording_config_id");
	seperated.push("visibility");
	seperated.push("stream_key");
	seperated.push("tags");

	qb.push(") VALUES (");

	let transcoding_config_id = if let Some(transcoding_config_id) = &req.transcoding_config_id {
		scuffle_utils::database::query("SELECT * FROM transcoding_configs WHERE id = $1 AND organization_id = $2")
			.bind(transcoding_config_id.into_ulid())
			.bind(access_token.organization_id)
			.build()
			.fetch_optional(&client)
			.await
			.map_err(|err| {
				tracing::error!(err = %err, "failed to fetch transcoding config");
				Status::internal("failed to fetch transcoding config")
			})?
			.ok_or_else(|| Status::not_found("transcoding config not found"))?;

		Some(transcoding_config_id.into_ulid())
	} else {
		None
	};

	let recording_config_id = if let Some(recording_config_id) = &req.recording_config_id {
		scuffle_utils::database::query("SELECT * FROM recording_configs WHERE id = $1 AND organization_id = $2")
			.bind(recording_config_id.into_ulid())
			.bind(access_token.organization_id)
			.build()
			.fetch_optional(&client)
			.await
			.map_err(|err| {
				tracing::error!(err = %err, "failed to fetch recording config");
				Status::internal("failed to fetch recording config")
			})?
			.ok_or_else(|| Status::not_found("recording config not found"))?;

		Some(recording_config_id.into_ulid())
	} else {
		None
	};

	let visibility = pb::scuffle::video::v1::types::Visibility::try_from(req.visibility)
		.map_err(|_| Status::invalid_argument("invalid visibility value"))?;

	// The stream key is 32 characters long randomly generated string.
	let mut seperated = qb.separated(",");

	seperated.push_bind(Ulid::new());
	seperated.push_bind(access_token.organization_id);
	seperated.push_bind(transcoding_config_id);
	seperated.push_bind(recording_config_id);
	seperated.push_bind(Visibility::from(visibility));
	seperated.push_bind(create_stream_key());
	seperated.push_bind(utils::database::Json(req.tags.clone().unwrap_or_default().tags));

	qb.push(") RETURNING *");

	Ok(qb)
}

impl ApiRequest<RoomCreateResponse> for tonic::Request<RoomCreateRequest> {
	async fn process<G: ApiGlobal>(
		&self,
		global: &Arc<G>,
		access_token: &AccessToken,
	) -> tonic::Result<tonic::Response<RoomCreateResponse>> {
		let req = self.get_ref();

		validate(req)?;

		let client = global.db().get().await.map_err(|err| {
			tracing::error!(err = %err, "failed to get db client");
			Status::internal("internal server error")
		})?;

		let query = build_query(req, &client, access_token).await?;

		let result: video_common::database::Room = query.build_query_as().fetch_one(client).await.map_err(|err| {
			tracing::error!(err = %err, "failed to create {}", <RoomCreateRequest as TonicRequest>::Table::FRIENDLY_NAME);
			tonic::Status::internal(format!(
				"failed to create {}",
				<RoomCreateRequest as TonicRequest>::Table::FRIENDLY_NAME
			))
		})?;

		video_common::events::emit(
			global.nats(),
			&global.config().events.stream_name,
			access_token.organization_id,
			Target::Room,
			event::Event::Room(event::Room {
				room_id: Some(result.id.into()),
				event: Some(event::room::Event::Created(event::room::Created {})),
			}),
		)
		.await;

		Ok(tonic::Response::new(RoomCreateResponse {
			stream_key: result.stream_key.clone(),
			room: Some(result.into_proto()),
		}))
	}
}
