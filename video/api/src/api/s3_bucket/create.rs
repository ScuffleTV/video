use std::sync::Arc;

use pb::scuffle::video::v1::events_fetch_request::Target;
use pb::scuffle::video::v1::types::access_token_scope::Permission;
use pb::scuffle::video::v1::types::{event, Resource};
use pb::scuffle::video::v1::{S3BucketCreateRequest, S3BucketCreateResponse};
use tonic::Status;
use ulid::Ulid;
use video_common::database::{AccessToken, DatabaseTable};

use super::utils::{
	validate_access_key_id, validate_endpoint, validate_name, validate_public_url, validate_region,
	validate_secret_access_key,
};
use crate::api::utils::tags::validate_tags;
use crate::api::utils::{impl_request_scopes, ApiRequest, TonicRequest};
use crate::global::ApiGlobal;
use crate::ratelimit::RateLimitResource;

impl_request_scopes!(
	S3BucketCreateRequest,
	video_common::database::S3Bucket,
	(Resource::S3Bucket, Permission::Create),
	RateLimitResource::S3BucketCreate
);

pub fn validate(req: &S3BucketCreateRequest) -> tonic::Result<()> {
	validate_tags(req.tags.as_ref())
}

pub fn build_query<'a>(
	req: &'a S3BucketCreateRequest,
	access_token: &AccessToken,
) -> tonic::Result<utils::database::QueryBuilder<'a>> {
	let mut qb = scuffle_utils::database::QueryBuilder::default();

	qb.push("INSERT INTO ")
		.push(<S3BucketCreateRequest as TonicRequest>::Table::NAME)
		.push(" (");

	let mut seperated = qb.separated(",");

	seperated.push("id");
	seperated.push("organization_id");
	seperated.push("name");
	seperated.push("region");
	seperated.push("endpoint");
	seperated.push("access_key_id");
	seperated.push("secret_access_key");
	seperated.push("public_url");
	seperated.push("tags");
	seperated.push("managed");

	qb.push(") VALUES (");

	let mut seperated = qb.separated(",");

	// TODO: check if this is actually secure. How do we prevent SSRF?
	// How do we make sure that these urls point outside of our network?
	if let Some(endpoint) = &req.endpoint {
		validate_endpoint(endpoint)?;
	}

	if let Some(public_url) = &req.public_url {
		validate_public_url(public_url)?;
	}

	validate_name(&req.name)?;
	validate_region(&req.region)?;
	validate_access_key_id(&req.access_key_id)?;
	validate_secret_access_key(&req.secret_access_key)?;

	seperated.push_bind(Ulid::new());
	seperated.push_bind(access_token.organization_id);
	seperated.push_bind(&req.name);
	seperated.push_bind(&req.region);
	seperated.push_bind(&req.endpoint);
	seperated.push_bind(&req.access_key_id);
	seperated.push_bind(&req.secret_access_key);
	seperated.push_bind(&req.public_url);
	seperated.push_bind(utils::database::Json(req.tags.clone().unwrap_or_default().tags));
	seperated.push_bind(false);

	qb.push(") RETURNING *");

	Ok(qb)
}

impl ApiRequest<S3BucketCreateResponse> for tonic::Request<S3BucketCreateRequest> {
	async fn process<G: ApiGlobal>(
		&self,
		global: &Arc<G>,
		access_token: &AccessToken,
	) -> tonic::Result<tonic::Response<S3BucketCreateResponse>> {
		let req = self.get_ref();

		validate(req)?;

		let query_builder = build_query(req, access_token)?;

		let result: video_common::database::S3Bucket =
			query_builder.build_query_as().fetch_one(global.db()).await.map_err(|err| {
				tracing::error!(err = %err, "failed to create {}", <S3BucketCreateRequest as TonicRequest>::Table::FRIENDLY_NAME);
				Status::internal(format!(
					"failed to create {}",
					<S3BucketCreateRequest as TonicRequest>::Table::FRIENDLY_NAME
				))
			})?;

		video_common::events::emit(
			global.nats(),
			&global.config().events.stream_name,
			access_token.organization_id,
			Target::S3Bucket,
			event::Event::S3Bucket(event::S3Bucket {
				s3_bucket_id: Some(result.id.into()),
				event: Some(event::s3_bucket::Event::Created(event::s3_bucket::Created {})),
			}),
		)
		.await;

		Ok(tonic::Response::new(S3BucketCreateResponse {
			s3_bucket: Some(result.into_proto()),
		}))
	}
}
