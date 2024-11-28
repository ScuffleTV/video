use std::collections::HashMap;

use pb::scuffle::video::internal::{IngestWatchRequest, IngestWatchResponse};
use tokio::sync::{mpsc, Mutex};
use tonic::Streaming;
use ulid::Ulid;

use crate::config::IngestConfig;

pub struct IncomingTranscoder {
	pub ulid: Ulid,
	pub message: IngestWatchRequest,
	pub streaming: Streaming<IngestWatchRequest>,
	pub transcoder: mpsc::Sender<IngestWatchResponse>,
}

pub trait IngestState {
	fn requests(&self) -> &Mutex<HashMap<Ulid, mpsc::Sender<IncomingTranscoder>>>;
}

pub trait IngestGlobal:
	binary_helper::global::GlobalCtx
	+ binary_helper::global::GlobalConfigProvider<IngestConfig>
	+ binary_helper::global::GlobalNats
	+ binary_helper::global::GlobalDb
	+ binary_helper::global::GlobalConfig
	+ IngestState
	+ Send
	+ Sync
	+ 'static
{
}

impl<T> IngestGlobal for T where
	T: binary_helper::global::GlobalCtx
		+ binary_helper::global::GlobalConfigProvider<IngestConfig>
		+ binary_helper::global::GlobalNats
		+ binary_helper::global::GlobalDb
		+ binary_helper::global::GlobalConfig
		+ IngestState
		+ Send
		+ Sync
		+ 'static
{
}
