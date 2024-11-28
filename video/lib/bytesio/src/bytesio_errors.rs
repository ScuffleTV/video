use std::fmt;

use tokio::time::error::Elapsed;

#[derive(Debug)]
pub enum BytesIOError {
	Timeout,
	ClientClosed,
}

impl From<Elapsed> for BytesIOError {
	fn from(_error: tokio::time::error::Elapsed) -> Self {
		Self::Timeout
	}
}

impl fmt::Display for BytesIOError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Self::Timeout => write!(f, "timeout"),
			Self::ClientClosed => write!(f, "client closed"),
		}
	}
}
