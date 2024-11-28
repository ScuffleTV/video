use std::fmt;

use amf0::Amf0WriteError;

use crate::chunk::ChunkEncodeError;
use crate::macros::from_error;

#[derive(Debug)]
pub enum NetConnectionError {
	Amf0Write(Amf0WriteError),
	ChunkEncode(ChunkEncodeError),
}

from_error!(NetConnectionError, Self::Amf0Write, Amf0WriteError);
from_error!(NetConnectionError, Self::ChunkEncode, ChunkEncodeError);

impl fmt::Display for NetConnectionError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Self::Amf0Write(err) => write!(f, "amf0 write error: {}", err),
			Self::ChunkEncode(err) => write!(f, "chunk encode error: {}", err),
		}
	}
}
