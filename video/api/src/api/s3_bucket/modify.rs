use std::sync::Arc;

use pb::ext::UlidExt;
use pb::scuffle::video::v1::events_fetch_request::Target;
use pb::scuffle::video::v1::types::access_token_scope::Permission;
use pb::scuffle::video::v1::types::{event, Resource};
use pb::scuffle::video::v1::{S3BucketModifyRequest, S3BucketModifyResponse};
use video_common::database::{AccessToken, DatabaseTable};

use super::utils::{
	validate_access_key_id, validate_endpoint, validate_name, validate_public_url, validate_region,
	validate_secret_access_key,
};
use crate::api::errors::MODIFY_NO_FIELDS;
use crate::api::utils::tags::validate_tags;
use crate::api::utils::{impl_request_scopes, ApiRequest, TonicRequest};
use crate::global::ApiGlobal;
use crate::ratelimit::RateLimitResource;

impl_request_scopes!(
	S3BucketModifyRequest,
	video_common::database::S3Bucket,
	(Resource::S3Bucket, Permission::Modify),
	RateLimitResource::S3BucketModify
);

pub fn validate(req: &S3BucketModifyRequest) -> tonic::Result<()> {
	validate_tags(req.tags.as_ref())
}

pub fn build_query<'a>(
	req: &'a S3BucketModifyRequest,
	access_token: &AccessToken,
) -> tonic::Result<utils::database::QueryBuilder<'a>> {
	let mut qb = scuffle_utils::database::QueryBuilder::default();

	qb.push("UPDATE ")
		.push(<S3BucketModifyRequest as TonicRequest>::Table::NAME)
		.push(" SET ");

	let mut seperated = qb.separated(",");

	if let Some(access_key_id) = &req.access_key_id {
		validate_access_key_id(access_key_id)?;
		seperated.push("access_key_id = ").push_bind_unseparated(access_key_id);
	}

	if let Some(secret_access_key) = &req.secret_access_key {
		validate_secret_access_key(secret_access_key)?;
		seperated
			.push("secret_access_key = ")
			.push_bind_unseparated(secret_access_key);
	}

	if let Some(name) = &req.name {
		validate_name(name)?;
		seperated.push("name = ").push_bind_unseparated(name);
	}

	if let Some(region) = &req.region {
		validate_region(region)?;
		seperated.push("region = ").push_bind_unseparated(region);
	}

	if let Some(endpoint) = &req.endpoint {
		if endpoint.is_empty() {
			seperated.push("endpoint = NULL");
		} else {
			validate_endpoint(endpoint)?;
			seperated.push("endpoint = ").push_bind_unseparated(endpoint);
		}
	}

	if let Some(public_url) = &req.public_url {
		if public_url.is_empty() {
			seperated.push("public_url = NULL");
		} else {
			validate_public_url(public_url)?;
			seperated.push("public_url = ").push_bind_unseparated(public_url);
		}
	}

	if let Some(tags) = &req.tags {
		seperated
			.push("tags = ")
			.push_bind_unseparated(utils::database::Json(&tags.tags));
	}

	if req.tags.is_none()
		&& req.access_key_id.is_none()
		&& req.secret_access_key.is_none()
		&& req.name.is_none()
		&& req.region.is_none()
		&& req.endpoint.is_none()
		&& req.public_url.is_none()
	{
		return Err(tonic::Status::invalid_argument(MODIFY_NO_FIELDS));
	}

	seperated.push("updated_at = NOW()");

	qb.push(" WHERE id = ").push_bind(req.id.into_ulid());
	qb.push(" AND organization_id = ").push_bind(access_token.organization_id);
	qb.push(" RETURNING *");

	Ok(qb)
}

impl ApiRequest<S3BucketModifyResponse> for tonic::Request<S3BucketModifyRequest> {
	async fn process<G: ApiGlobal>(
		&self,
		global: &Arc<G>,
		access_token: &AccessToken,
	) -> tonic::Result<tonic::Response<S3BucketModifyResponse>> {
		let req = self.get_ref();

		validate(req)?;

		let query = build_query(req, access_token)?;

		let result: Option<video_common::database::S3Bucket> =
			query.build_query_as().fetch_optional(global.db()).await.map_err(|err| {
				tracing::error!(err = %err, "failed to modify {}", <S3BucketModifyRequest as TonicRequest>::Table::FRIENDLY_NAME);
				tonic::Status::internal(format!(
					"failed to modify {}",
					<S3BucketModifyRequest as TonicRequest>::Table::FRIENDLY_NAME
				))
			})?;

		match result {
			Some(result) => {
				video_common::events::emit(
					global.nats(),
					&global.config().events.stream_name,
					access_token.organization_id,
					Target::S3Bucket,
					event::Event::S3Bucket(event::S3Bucket {
						s3_bucket_id: Some(result.id.into()),
						event: Some(event::s3_bucket::Event::Modified(event::s3_bucket::Modified {})),
					}),
				)
				.await;
				Ok(tonic::Response::new(S3BucketModifyResponse {
					s3_bucket: Some(result.into_proto()),
				}))
			}
			None => Err(tonic::Status::not_found(format!(
				"{} not found",
				<S3BucketModifyRequest as TonicRequest>::Table::FRIENDLY_NAME
			))),
		}
	}
}
