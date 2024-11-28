use pb::scuffle::video::v1::playback_key_pair_server::{
	PlaybackKeyPair as PlaybackKeyPairServiceTrait, PlaybackKeyPairServer as PlaybackKeyPairService,
};
use pb::scuffle::video::v1::{
	PlaybackKeyPairCreateRequest, PlaybackKeyPairCreateResponse, PlaybackKeyPairDeleteRequest,
	PlaybackKeyPairDeleteResponse, PlaybackKeyPairGetRequest, PlaybackKeyPairGetResponse, PlaybackKeyPairModifyRequest,
	PlaybackKeyPairModifyResponse, PlaybackKeyPairTagRequest, PlaybackKeyPairTagResponse, PlaybackKeyPairUntagRequest,
	PlaybackKeyPairUntagResponse,
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

pub struct PlaybackKeyPairServer<G: ApiGlobal> {
	_phantom: std::marker::PhantomData<G>,
}

impl<G: ApiGlobal> PlaybackKeyPairServer<G> {
	pub fn build() -> PlaybackKeyPairService<Self> {
		PlaybackKeyPairService::new(Self::new())
	}

	pub(crate) const fn new() -> Self {
		Self {
			_phantom: std::marker::PhantomData,
		}
	}
}

#[async_trait]
impl<G: ApiGlobal> PlaybackKeyPairServiceTrait for PlaybackKeyPairServer<G> {
	async fn get(&self, request: Request<PlaybackKeyPairGetRequest>) -> tonic::Result<Response<PlaybackKeyPairGetResponse>> {
		scope_ratelimit!(self, request, global, access_token, || async {
			request.process(global, access_token).await
		});
	}

	async fn create(
		&self,
		request: Request<PlaybackKeyPairCreateRequest>,
	) -> tonic::Result<Response<PlaybackKeyPairCreateResponse>> {
		scope_ratelimit!(self, request, global, access_token, || async {
			request.process(global, access_token).await
		});
	}

	async fn modify(
		&self,
		request: Request<PlaybackKeyPairModifyRequest>,
	) -> tonic::Result<Response<PlaybackKeyPairModifyResponse>> {
		scope_ratelimit!(self, request, global, access_token, || async {
			request.process(global, access_token).await
		});
	}

	async fn delete(
		&self,
		request: Request<PlaybackKeyPairDeleteRequest>,
	) -> tonic::Result<Response<PlaybackKeyPairDeleteResponse>> {
		scope_ratelimit!(self, request, global, access_token, || async {
			request.process(global, access_token).await
		});
	}

	async fn tag(&self, request: Request<PlaybackKeyPairTagRequest>) -> tonic::Result<Response<PlaybackKeyPairTagResponse>> {
		scope_ratelimit!(self, request, global, access_token, || async {
			request.process(global, access_token).await
		});
	}

	async fn untag(
		&self,
		request: Request<PlaybackKeyPairUntagRequest>,
	) -> tonic::Result<Response<PlaybackKeyPairUntagResponse>> {
		scope_ratelimit!(self, request, global, access_token, || async {
			request.process(global, access_token).await
		});
	}
}
