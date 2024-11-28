use pb::scuffle::video::v1::recording_server::{Recording as RecordingServiceTrait, RecordingServer as RecordingService};
use pb::scuffle::video::v1::{
	RecordingDeleteRequest, RecordingDeleteResponse, RecordingGetRequest, RecordingGetResponse, RecordingModifyRequest,
	RecordingModifyResponse, RecordingTagRequest, RecordingTagResponse, RecordingUntagRequest, RecordingUntagResponse,
};
use tonic::{async_trait, Request, Response};

use super::utils::ratelimit::scope_ratelimit;
use super::utils::ApiRequest;
use crate::global::ApiGlobal;

pub(crate) mod delete;
pub(crate) mod get;
pub(crate) mod modify;
pub(crate) mod tag;
pub(crate) mod untag;

pub struct RecordingServer<G: ApiGlobal> {
	_phantom: std::marker::PhantomData<G>,
}

impl<G: ApiGlobal> RecordingServer<G> {
	pub fn build() -> RecordingService<Self> {
		RecordingService::new(Self::new())
	}

	pub(crate) const fn new() -> Self {
		Self {
			_phantom: std::marker::PhantomData,
		}
	}
}

#[async_trait]
impl<G: ApiGlobal> RecordingServiceTrait for RecordingServer<G> {
	async fn get(&self, request: Request<RecordingGetRequest>) -> tonic::Result<Response<RecordingGetResponse>> {
		scope_ratelimit!(self, request, global, access_token, || async {
			request.process(global, access_token).await
		});
	}

	async fn modify(&self, request: Request<RecordingModifyRequest>) -> tonic::Result<Response<RecordingModifyResponse>> {
		scope_ratelimit!(self, request, global, access_token, || async {
			request.process(global, access_token).await
		});
	}

	async fn delete(&self, request: Request<RecordingDeleteRequest>) -> tonic::Result<Response<RecordingDeleteResponse>> {
		scope_ratelimit!(self, request, global, access_token, || async {
			request.process(global, access_token).await
		});
	}

	async fn tag(&self, request: Request<RecordingTagRequest>) -> tonic::Result<Response<RecordingTagResponse>> {
		scope_ratelimit!(self, request, global, access_token, || async {
			request.process(global, access_token).await
		});
	}

	async fn untag(&self, request: Request<RecordingUntagRequest>) -> tonic::Result<Response<RecordingUntagResponse>> {
		scope_ratelimit!(self, request, global, access_token, || async {
			request.process(global, access_token).await
		});
	}
}
