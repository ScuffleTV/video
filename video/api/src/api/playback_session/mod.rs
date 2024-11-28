use pb::scuffle::video::v1::playback_session_server::{
	PlaybackSession as PlaybackSessionServiceTrait, PlaybackSessionServer as PlaybackSessionService,
};
use pb::scuffle::video::v1::{
	PlaybackSessionCountRequest, PlaybackSessionCountResponse, PlaybackSessionGetRequest, PlaybackSessionGetResponse,
	PlaybackSessionRevokeRequest, PlaybackSessionRevokeResponse,
};
use tonic::{async_trait, Request, Response};

use super::utils::ratelimit::scope_ratelimit;
use super::utils::ApiRequest;
use crate::global::ApiGlobal;

pub(crate) mod count;
pub(crate) mod get;
pub(crate) mod revoke;

pub struct PlaybackSessionServer<G: ApiGlobal> {
	_phantom: std::marker::PhantomData<G>,
}

impl<G: ApiGlobal> PlaybackSessionServer<G> {
	pub fn build() -> PlaybackSessionService<Self> {
		PlaybackSessionService::new(Self::new())
	}

	pub(crate) const fn new() -> Self {
		Self {
			_phantom: std::marker::PhantomData,
		}
	}
}

#[async_trait]
impl<G: ApiGlobal> PlaybackSessionServiceTrait for PlaybackSessionServer<G> {
	async fn get(&self, request: Request<PlaybackSessionGetRequest>) -> tonic::Result<Response<PlaybackSessionGetResponse>> {
		scope_ratelimit!(self, request, global, access_token, || async {
			request.process(global, access_token).await
		});
	}

	async fn revoke(
		&self,
		request: Request<PlaybackSessionRevokeRequest>,
	) -> tonic::Result<Response<PlaybackSessionRevokeResponse>> {
		scope_ratelimit!(self, request, global, access_token, || async {
			request.process(global, access_token).await
		});
	}

	async fn count(
		&self,
		request: Request<PlaybackSessionCountRequest>,
	) -> tonic::Result<Response<PlaybackSessionCountResponse>> {
		scope_ratelimit!(self, request, global, access_token, || async {
			request.process(global, access_token).await
		});
	}
}
