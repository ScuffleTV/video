use std::collections::HashSet;
use std::sync::Arc;

use pb::scuffle::video::v1::events_fetch_request::Target;
use pb::scuffle::video::v1::types::access_token_scope::Permission;
use pb::scuffle::video::v1::types::{event, Resource};
use pb::scuffle::video::v1::{TranscodingConfigCreateRequest, TranscodingConfigCreateResponse};
use tonic::Status;
use ulid::Ulid;
use video_common::database::{AccessToken, DatabaseTable, Rendition};

use crate::api::utils::tags::validate_tags;
use crate::api::utils::{impl_request_scopes, ApiRequest, TonicRequest};
use crate::global::ApiGlobal;
use crate::ratelimit::RateLimitResource;

impl_request_scopes!(
	TranscodingConfigCreateRequest,
	video_common::database::TranscodingConfig,
	(Resource::TranscodingConfig, Permission::Create),
	RateLimitResource::TranscodingConfigCreate
);

pub fn validate(req: &TranscodingConfigCreateRequest) -> tonic::Result<()> {
	validate_tags(req.tags.as_ref())
}

pub fn build_query(
	req: &TranscodingConfigCreateRequest,
	access_token: &AccessToken,
) -> tonic::Result<utils::database::QueryBuilder<'static>> {
	let mut qb = scuffle_utils::database::QueryBuilder::default();

	qb.push("INSERT INTO ")
		.push(<TranscodingConfigCreateRequest as TonicRequest>::Table::NAME)
		.push(" (");

	let mut seperated = qb.separated(",");

	seperated.push("id");
	seperated.push("organization_id");
	seperated.push("renditions");
	seperated.push("tags");

	qb.push(") VALUES (");

	let mut seperated = qb.separated(",");

	let renditions = req.renditions().map(Rendition::from).collect::<HashSet<_>>();

	if !renditions.iter().any(|r| r.is_audio()) {
		return Err(Status::invalid_argument("must specify at least one audio rendition"));
	}

	if !renditions.iter().any(|r| r.is_video()) {
		return Err(Status::invalid_argument("must specify at least one video rendition"));
	}

	seperated.push_bind(Ulid::new());
	seperated.push_bind(access_token.organization_id);
	seperated.push_bind(renditions.into_iter().collect::<Vec<_>>());
	seperated.push_bind(utils::database::Json(req.tags.clone().unwrap_or_default().tags));

	qb.push(") RETURNING *");

	Ok(qb)
}

impl ApiRequest<TranscodingConfigCreateResponse> for tonic::Request<TranscodingConfigCreateRequest> {
	async fn process<G: ApiGlobal>(
		&self,
		global: &Arc<G>,
		access_token: &AccessToken,
	) -> tonic::Result<tonic::Response<TranscodingConfigCreateResponse>> {
		let req = self.get_ref();

		validate(req)?;

		let query = build_query(req, access_token)?;

		let result: video_common::database::TranscodingConfig =
			query.build_query_as().fetch_one(global.db()).await.map_err(|err| {
				tracing::error!(err = %err, "failed to create {}", <TranscodingConfigCreateRequest as TonicRequest>::Table::FRIENDLY_NAME);
				tonic::Status::internal(format!(
					"failed to create {}",
					<TranscodingConfigCreateRequest as TonicRequest>::Table::FRIENDLY_NAME
				))
			})?;

		video_common::events::emit(
			global.nats(),
			&global.config().events.stream_name,
			access_token.organization_id,
			Target::TranscodingConfig,
			event::Event::TranscodingConfig(event::TranscodingConfig {
				transcoding_config_id: Some(result.id.into()),
				event: Some(event::transcoding_config::Event::Created(
					event::transcoding_config::Created {},
				)),
			}),
		)
		.await;

		Ok(tonic::Response::new(TranscodingConfigCreateResponse {
			transcoding_config: Some(result.into_proto()),
		}))
	}
}
