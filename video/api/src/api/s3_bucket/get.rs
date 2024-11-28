use std::sync::Arc;

use pb::scuffle::video::v1::types::access_token_scope::Permission;
use pb::scuffle::video::v1::types::Resource;
use pb::scuffle::video::v1::{S3BucketGetRequest, S3BucketGetResponse};
use video_common::database::{AccessToken, DatabaseTable};

use crate::api::utils::{get, impl_request_scopes, ApiRequest, TonicRequest};
use crate::global::ApiGlobal;
use crate::ratelimit::RateLimitResource;

impl_request_scopes!(
	S3BucketGetRequest,
	video_common::database::S3Bucket,
	(Resource::S3Bucket, Permission::Read),
	RateLimitResource::S3BucketGet
);

pub fn build_query(
	req: &S3BucketGetRequest,
	access_token: &AccessToken,
) -> tonic::Result<utils::database::QueryBuilder<'static>> {
	let mut qb = scuffle_utils::database::QueryBuilder::default();
	qb.push("SELECT * FROM ")
		.push(<S3BucketGetRequest as TonicRequest>::Table::NAME)
		.push(" WHERE ");
	let mut seperated = qb.separated(" AND ");

	get::organization_id(&mut seperated, access_token.organization_id);
	get::ids(&mut seperated, &req.ids);
	get::search_options(&mut seperated, req.search_options.as_ref())?;

	Ok(qb)
}

impl ApiRequest<S3BucketGetResponse> for tonic::Request<S3BucketGetRequest> {
	async fn process<G: ApiGlobal>(
		&self,
		global: &Arc<G>,
		access_token: &AccessToken,
	) -> tonic::Result<tonic::Response<S3BucketGetResponse>> {
		let req = self.get_ref();

		let query = build_query(req, access_token)?;

		let results = query.build_query_as().fetch_all(global.db()).await.map_err(|err| {
			tracing::error!(err = %err, "failed to fetch {}s", <S3BucketGetRequest as TonicRequest>::Table::FRIENDLY_NAME);
			tonic::Status::internal(format!(
				"failed to fetch {}s",
				<S3BucketGetRequest as TonicRequest>::Table::FRIENDLY_NAME
			))
		})?;

		Ok(tonic::Response::new(S3BucketGetResponse {
			s3_buckets: results
				.into_iter()
				.map(video_common::database::S3Bucket::into_proto)
				.collect(),
		}))
	}
}
