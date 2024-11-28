use std::{fmt, io};

use crate::macros::from_error;

#[derive(Debug)]
pub enum ChunkDecodeError {
	IO(io::Error),
	InvalidChunkType(u8),
	InvalidMessageTypeID(u8),
	MissingPreviousChunkHeader(u32),
	TooManyPartialChunks,
	TooManyPreviousChunkHeaders,
	PartialChunkTooLarge(usize),
	TimestampOverflow(u32, u32),
}

from_error!(ChunkDecodeError, Self::IO, io::Error);

#[derive(Debug)]
pub enum ChunkEncodeError {
	UnknownReadState,
	IO(io::Error),
}

from_error!(ChunkEncodeError, Self::IO, io::Error);

impl fmt::Display for ChunkEncodeError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Self::UnknownReadState => write!(f, "unknown read state"),
			Self::IO(err) => write!(f, "io error: {}", err),
		}
	}
}

impl fmt::Display for ChunkDecodeError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Self::IO(err) => write!(f, "io error: {}", err),
			Self::TooManyPartialChunks => write!(f, "too many partial chunks"),
			Self::TooManyPreviousChunkHeaders => write!(f, "too many previous chunk headers"),
			Self::PartialChunkTooLarge(size) => write!(f, "partial chunk too large: {}", size),
			Self::MissingPreviousChunkHeader(chunk_stream_id) => {
				write!(f, "missing previous chunk header: {}", chunk_stream_id)
			}
			Self::InvalidMessageTypeID(message_type_id) => {
				write!(f, "invalid message type id: {}", message_type_id)
			}
			Self::InvalidChunkType(chunk_type) => {
				write!(f, "invalid chunk type: {}", chunk_type)
			}
			Self::TimestampOverflow(timestamp, delta) => {
				write!(f, "timestamp overflow: timestamp: {}, delta: {}", timestamp, delta)
			}
		}
	}
}
