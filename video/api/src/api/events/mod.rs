use pb::scuffle::video::v1::events_server::{Events, EventsServer as EventsService};
use pb::scuffle::video::v1::{EventsAckRequest, EventsAckResponse, EventsFetchRequest};
use tonic::{async_trait, Request, Response};

use super::utils::ratelimit::scope_ratelimit;
use crate::api::utils::ApiRequest;
use crate::global::ApiGlobal;

pub struct EventsServer<G: ApiGlobal> {
	_phantom: std::marker::PhantomData<G>,
}

mod ack;
mod fetch;
mod utils;

impl<G: ApiGlobal> EventsServer<G> {
	pub fn build() -> EventsService<Self> {
		EventsService::new(Self::new())
	}

	pub(crate) const fn new() -> Self {
		Self {
			_phantom: std::marker::PhantomData,
		}
	}
}

#[async_trait]
impl<G: ApiGlobal> Events for EventsServer<G> {
	type FetchStream = fetch::Stream;

	async fn ack(&self, request: Request<EventsAckRequest>) -> tonic::Result<Response<EventsAckResponse>> {
		scope_ratelimit!(self, request, global, access_token, || async {
			request.process(global, access_token).await
		});
	}

	async fn fetch(&self, request: Request<EventsFetchRequest>) -> tonic::Result<Response<Self::FetchStream>> {
		scope_ratelimit!(self, request, global, access_token, || async {
			request.process(global, access_token).await
		});
	}
}
