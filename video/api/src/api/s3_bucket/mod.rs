use pb::scuffle::video::v1::s3_bucket_server::{S3Bucket as S3BucketServiceTrait, S3BucketServer as S3BucketService};
use pb::scuffle::video::v1::{
	S3BucketCreateRequest, S3BucketCreateResponse, S3BucketDeleteRequest, S3BucketDeleteResponse, S3BucketGetRequest,
	S3BucketGetResponse, S3BucketModifyRequest, S3BucketModifyResponse, S3BucketTagRequest, S3BucketTagResponse,
	S3BucketUntagRequest, S3BucketUntagResponse,
};
use tonic::{async_trait, Request, Response};

use super::utils::ratelimit::scope_ratelimit;
use super::utils::ApiRequest;
use crate::global::ApiGlobal;

pub(crate) mod create;
pub(crate) mod delete;
pub(crate) mod get;
pub(crate) mod modify;
pub(crate) mod tag;
pub(crate) mod untag;
pub(crate) mod utils;

pub struct S3BucketServer<G: ApiGlobal> {
	_phantom: std::marker::PhantomData<G>,
}

impl<G: ApiGlobal> S3BucketServer<G> {
	pub fn build() -> S3BucketService<Self> {
		S3BucketService::new(Self::new())
	}

	pub(crate) const fn new() -> Self {
		Self {
			_phantom: std::marker::PhantomData,
		}
	}
}

#[async_trait]
impl<G: ApiGlobal> S3BucketServiceTrait for S3BucketServer<G> {
	async fn get(&self, request: Request<S3BucketGetRequest>) -> tonic::Result<Response<S3BucketGetResponse>> {
		scope_ratelimit!(self, request, global, access_token, || async {
			request.process(global, access_token).await
		});
	}

	async fn create(&self, request: Request<S3BucketCreateRequest>) -> tonic::Result<Response<S3BucketCreateResponse>> {
		scope_ratelimit!(self, request, global, access_token, || async {
			request.process(global, access_token).await
		});
	}

	async fn modify(&self, request: Request<S3BucketModifyRequest>) -> tonic::Result<Response<S3BucketModifyResponse>> {
		scope_ratelimit!(self, request, global, access_token, || async {
			request.process(global, access_token).await
		});
	}

	async fn delete(&self, request: Request<S3BucketDeleteRequest>) -> tonic::Result<Response<S3BucketDeleteResponse>> {
		scope_ratelimit!(self, request, global, access_token, || async {
			request.process(global, access_token).await
		});
	}

	async fn tag(&self, request: Request<S3BucketTagRequest>) -> tonic::Result<Response<S3BucketTagResponse>> {
		scope_ratelimit!(self, request, global, access_token, || async {
			request.process(global, access_token).await
		});
	}

	async fn untag(&self, request: Request<S3BucketUntagRequest>) -> tonic::Result<Response<S3BucketUntagResponse>> {
		scope_ratelimit!(self, request, global, access_token, || async {
			request.process(global, access_token).await
		});
	}
}
