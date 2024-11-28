use pb::scuffle::video::v1::access_token_server::{
	AccessToken as AccessTokenServiceTrait, AccessTokenServer as AccessTokenService,
};
use pb::scuffle::video::v1::{
	AccessTokenCreateRequest, AccessTokenCreateResponse, AccessTokenDeleteRequest, AccessTokenDeleteResponse,
	AccessTokenGetRequest, AccessTokenGetResponse, AccessTokenTagRequest, AccessTokenTagResponse, AccessTokenUntagRequest,
	AccessTokenUntagResponse,
};
use tonic::{async_trait, Request, Response};

use super::utils::ratelimit::scope_ratelimit;
use super::utils::ApiRequest;
use crate::global::ApiGlobal;

pub(crate) mod create;
pub(crate) mod delete;
pub(crate) mod get;
pub(crate) mod tag;
pub(crate) mod untag;

pub struct AccessTokenServer<G: ApiGlobal> {
	_phantom: std::marker::PhantomData<G>,
}

impl<G: ApiGlobal> AccessTokenServer<G> {
	pub fn build() -> AccessTokenService<Self> {
		AccessTokenService::new(Self::new())
	}

	pub(crate) const fn new() -> Self {
		Self {
			_phantom: std::marker::PhantomData,
		}
	}
}

#[async_trait]
impl<G: ApiGlobal> AccessTokenServiceTrait for AccessTokenServer<G> {
	async fn get(&self, request: Request<AccessTokenGetRequest>) -> tonic::Result<Response<AccessTokenGetResponse>> {
		scope_ratelimit!(self, request, global, access_token, || async {
			request.process(global, access_token).await
		});
	}

	async fn create(
		&self,
		request: Request<AccessTokenCreateRequest>,
	) -> tonic::Result<Response<AccessTokenCreateResponse>> {
		scope_ratelimit!(self, request, global, access_token, || async {
			request.process(global, access_token).await
		});
	}

	async fn delete(
		&self,
		request: Request<AccessTokenDeleteRequest>,
	) -> tonic::Result<Response<AccessTokenDeleteResponse>> {
		scope_ratelimit!(self, request, global, access_token, || async {
			request.process(global, access_token).await
		});
	}

	async fn tag(&self, request: Request<AccessTokenTagRequest>) -> tonic::Result<Response<AccessTokenTagResponse>> {
		scope_ratelimit!(self, request, global, access_token, || async {
			request.process(global, access_token).await
		});
	}

	async fn untag(&self, request: Request<AccessTokenUntagRequest>) -> tonic::Result<Response<AccessTokenUntagResponse>> {
		scope_ratelimit!(self, request, global, access_token, || async {
			request.process(global, access_token).await
		});
	}
}
