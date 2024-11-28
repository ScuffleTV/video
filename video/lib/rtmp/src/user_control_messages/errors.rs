use std::fmt;

use crate::chunk::ChunkEncodeError;
use crate::macros::from_error;

#[derive(Debug)]
pub enum EventMessagesError {
	ChunkEncode(ChunkEncodeError),
}

from_error!(EventMessagesError, Self::ChunkEncode, ChunkEncodeError);

impl fmt::Display for EventMessagesError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match &self {
			Self::ChunkEncode(e) => {
				write!(f, "chunk encode error: {}", e)
			}
		}
	}
}
