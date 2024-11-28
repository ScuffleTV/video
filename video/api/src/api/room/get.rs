use std::sync::Arc;

use pb::scuffle::video::v1::types::access_token_scope::Permission;
use pb::scuffle::video::v1::types::Resource;
use pb::scuffle::video::v1::{RoomGetRequest, RoomGetResponse};
use tonic::Status;
use video_common::database::{AccessToken, DatabaseTable, RoomStatus, Visibility};

use crate::api::utils::{get, impl_request_scopes, ApiRequest, TonicRequest};
use crate::global::ApiGlobal;
use crate::ratelimit::RateLimitResource;

impl_request_scopes!(
	RoomGetRequest,
	video_common::database::Room,
	(Resource::Room, Permission::Read),
	RateLimitResource::RoomGet
);

pub fn build_query(
	req: &RoomGetRequest,
	access_token: &AccessToken,
) -> tonic::Result<utils::database::QueryBuilder<'static>> {
	let mut qb = scuffle_utils::database::QueryBuilder::default();
	qb.push("SELECT * FROM ")
		.push(<RoomGetRequest as TonicRequest>::Table::NAME)
		.push(" WHERE ");
	let mut seperated = qb.separated(" AND ");

	get::organization_id(&mut seperated, access_token.organization_id);
	get::ids(&mut seperated, &req.ids);

	if let Some(transcoding_config_id) = req.transcoding_config_id.as_ref() {
		seperated.push("transcoding_config_id = ");
		seperated.push_bind_unseparated(transcoding_config_id.into_ulid());
	}

	if let Some(recording_config_id) = req.recording_config_id.as_ref() {
		seperated.push("recording_config_id = ");
		seperated.push_bind_unseparated(recording_config_id.into_ulid());
	}

	if let Some(status) = req.status {
		let status = pb::scuffle::video::v1::types::RoomStatus::try_from(status)
			.map_err(|_| Status::invalid_argument("invalid status value"))?;

		seperated.push("status = ");
		seperated.push_bind_unseparated(RoomStatus::from(status));
	}

	if let Some(visibility) = req.visibility {
		let visibility = pb::scuffle::video::v1::types::Visibility::try_from(visibility)
			.map_err(|_| Status::invalid_argument("invalid visibility value"))?;

		seperated.push("visibility = ");
		seperated.push_bind_unseparated(Visibility::from(visibility));
	}

	get::search_options(&mut seperated, req.search_options.as_ref())?;

	Ok(qb)
}

impl ApiRequest<RoomGetResponse> for tonic::Request<RoomGetRequest> {
	async fn process<G: ApiGlobal>(
		&self,
		global: &Arc<G>,
		access_token: &AccessToken,
	) -> tonic::Result<tonic::Response<RoomGetResponse>> {
		let req = self.get_ref();

		let query_builder = build_query(req, access_token)?;

		let results: Vec<video_common::database::Room> =
			query_builder.build_query_as().fetch_all(global.db()).await.map_err(|err| {
				tracing::error!(err = %err, "failed to fetch {}s", <RoomGetRequest as TonicRequest>::Table::FRIENDLY_NAME);
				Status::internal(format!(
					"failed to fetch {}s",
					<RoomGetRequest as TonicRequest>::Table::FRIENDLY_NAME
				))
			})?;

		Ok(tonic::Response::new(RoomGetResponse {
			rooms: results.into_iter().map(video_common::database::Room::into_proto).collect(),
		}))
	}
}
