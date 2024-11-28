use std::io;

use crate::chunk::ChunkEncodeError;
use crate::macros::from_error;

#[derive(Debug)]
pub enum ProtocolControlMessageError {
	IO(io::Error),
	ChunkEncode(ChunkEncodeError),
}

from_error!(ProtocolControlMessageError, Self::IO, io::Error);
from_error!(ProtocolControlMessageError, Self::ChunkEncode, ChunkEncodeError);

impl std::fmt::Display for ProtocolControlMessageError {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		match self {
			Self::IO(e) => write!(f, "io error: {}", e),
			Self::ChunkEncode(e) => write!(f, "chunk encode error: {}", e),
		}
	}
}
