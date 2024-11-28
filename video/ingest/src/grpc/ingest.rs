use std::pin::Pin;
use std::sync::{Arc, Weak};
use std::time::Duration;

use async_stream::try_stream;
use futures_util::Stream;
use pb::ext::UlidExt;
use pb::scuffle::video::internal::{ingest_server, ingest_watch_request, IngestWatchRequest, IngestWatchResponse};
use scuffle_utils::prelude::FutureTimeout;
use tonic::{async_trait, Request, Response, Status, Streaming};

use crate::global::{IncomingTranscoder, IngestGlobal};

pub struct IngestServer<G: IngestGlobal> {
	global: Weak<G>,
}

impl<G: IngestGlobal> IngestServer<G> {
	pub fn new(global: &Arc<G>) -> ingest_server::IngestServer<Self> {
		ingest_server::IngestServer::new(Self {
			global: Arc::downgrade(global),
		})
	}
}

type Result<T> = std::result::Result<T, Status>;

#[async_trait]
impl<G: IngestGlobal> ingest_server::Ingest for IngestServer<G> {
	/// Server streaming response type for the Watch method.
	type WatchStream = Pin<Box<dyn Stream<Item = Result<IngestWatchResponse>> + Send + Sync>>;

	async fn watch(&self, request: Request<Streaming<IngestWatchRequest>>) -> Result<Response<Self::WatchStream>> {
		let global = self
			.global
			.upgrade()
			.ok_or_else(|| Status::internal("Global state was dropped, cannot handle ingest request"))?;

		let mut request = request.into_inner();

		let Some(message) = request.message().await? else {
			return Err(Status::invalid_argument("No message provided"));
		};

		let open_req = match &message.message {
			Some(ingest_watch_request::Message::Open(message)) => message,
			Some(_) => return Err(Status::invalid_argument("Invalid message type")),
			None => return Err(Status::invalid_argument("No message provided")),
		};

		let ulid = open_req.request_id.into_ulid();

		let Some(handler) = global.requests().lock().await.remove(&ulid) else {
			return Err(Status::not_found("No ingest request found with that UUID"));
		};

		let (tx, mut rx) = tokio::sync::mpsc::channel(16);

		handler
			.send(IncomingTranscoder {
				ulid,
				message,
				streaming: request,
				transcoder: tx,
			})
			.await
			.map_err(|_| Status::internal("Failed to send request to handler"))?;

		let Ok(Some(message)) = rx.recv().timeout(Duration::from_secs(1)).await else {
			return Err(Status::internal("Failed to receive response from handler"));
		};

		let output = try_stream!({
			yield message;

			while let Some(message) = rx.recv().await {
				yield message;
			}
		});

		Ok(Response::new(Box::pin(output)))
	}
}
