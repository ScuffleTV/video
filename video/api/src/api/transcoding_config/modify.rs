use std::collections::HashSet;
use std::sync::Arc;

use pb::ext::UlidExt;
use pb::scuffle::video::v1::events_fetch_request::Target;
use pb::scuffle::video::v1::types::access_token_scope::Permission;
use pb::scuffle::video::v1::types::{event, Resource};
use pb::scuffle::video::v1::{TranscodingConfigModifyRequest, TranscodingConfigModifyResponse};
use tonic::Status;
use video_common::database::{AccessToken, DatabaseTable, Rendition};

use crate::api::errors::MODIFY_NO_FIELDS;
use crate::api::utils::tags::validate_tags;
use crate::api::utils::{impl_request_scopes, ApiRequest, TonicRequest};
use crate::global::ApiGlobal;
use crate::ratelimit::RateLimitResource;

impl_request_scopes!(
	TranscodingConfigModifyRequest,
	video_common::database::TranscodingConfig,
	(Resource::TranscodingConfig, Permission::Modify),
	RateLimitResource::TranscodingConfigModify
);

pub fn validate(req: &TranscodingConfigModifyRequest) -> tonic::Result<()> {
	validate_tags(req.tags.as_ref())
}

pub fn build_query<'a>(
	req: &'a TranscodingConfigModifyRequest,
	access_token: &AccessToken,
) -> tonic::Result<utils::database::QueryBuilder<'a>> {
	let mut qb = scuffle_utils::database::QueryBuilder::default();

	qb.push("UPDATE ")
		.push(<TranscodingConfigModifyRequest as TonicRequest>::Table::NAME)
		.push(" SET ");

	let mut seperated = qb.separated(",");

	if let Some(renditions) = &req.renditions {
		let renditions = renditions.items().map(Rendition::from).collect::<HashSet<_>>();

		if !renditions.iter().any(|r| r.is_audio()) {
			return Err(Status::invalid_argument("must specify at least one audio rendition"));
		}

		if !renditions.iter().any(|r| r.is_video()) {
			return Err(Status::invalid_argument("must specify at least one video rendition"));
		}

		seperated
			.push("renditions = ")
			.push_bind_unseparated(renditions.into_iter().collect::<Vec<_>>());
	}

	if let Some(tags) = &req.tags {
		seperated
			.push("tags = ")
			.push_bind_unseparated(utils::database::Json(&tags.tags));
	}

	if req.renditions.is_none() && req.tags.is_none() {
		return Err(tonic::Status::invalid_argument(MODIFY_NO_FIELDS));
	}

	seperated.push("updated_at = NOW()");

	qb.push(" WHERE id = ").push_bind(req.id.into_ulid());
	qb.push(" AND organization_id = ").push_bind(access_token.organization_id);
	qb.push(" RETURNING *");

	Ok(qb)
}

impl ApiRequest<TranscodingConfigModifyResponse> for tonic::Request<TranscodingConfigModifyRequest> {
	async fn process<G: ApiGlobal>(
		&self,
		global: &Arc<G>,
		access_token: &AccessToken,
	) -> tonic::Result<tonic::Response<TranscodingConfigModifyResponse>> {
		let req = self.get_ref();

		validate(req)?;

		let query = build_query(req, access_token)?;

		let result: Option<video_common::database::TranscodingConfig> =
			query.build_query_as().fetch_optional(global.db()).await.map_err(|err| {
				tracing::error!(err = %err, "failed to modify {}", <TranscodingConfigModifyRequest as TonicRequest>::Table::FRIENDLY_NAME);
				Status::internal(format!(
					"failed to modify {}",
					<TranscodingConfigModifyRequest as TonicRequest>::Table::FRIENDLY_NAME
				))
			})?;

		match result {
			Some(result) => {
				video_common::events::emit(
					global.nats(),
					&global.config().events.stream_name,
					access_token.organization_id,
					Target::TranscodingConfig,
					event::Event::TranscodingConfig(event::TranscodingConfig {
						transcoding_config_id: Some(result.id.into()),
						event: Some(event::transcoding_config::Event::Modified(
							event::transcoding_config::Modified {},
						)),
					}),
				)
				.await;
				Ok(tonic::Response::new(TranscodingConfigModifyResponse {
					transcoding_config: Some(result.into_proto()),
				}))
			}
			None => Err(Status::not_found(format!(
				"{} not found",
				<TranscodingConfigModifyRequest as TonicRequest>::Table::FRIENDLY_NAME
			))),
		}
	}
}
