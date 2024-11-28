use std::{fmt, io};

#[derive(Debug)]
pub enum FlvDemuxerError {
	IO(io::Error),
	Amf0Read(amf0::Amf0ReadError),
	InvalidFlvHeader,
	InvalidScriptDataName,
	InvalidEnhancedPacketType(u8),
	InvalidSoundRate(u8),
	InvalidSoundSize(u8),
	InvalidSoundType(u8),
	InvalidFrameType(u8),
}

impl From<io::Error> for FlvDemuxerError {
	fn from(error: io::Error) -> Self {
		Self::IO(error)
	}
}

impl From<amf0::Amf0ReadError> for FlvDemuxerError {
	fn from(value: amf0::Amf0ReadError) -> Self {
		Self::Amf0Read(value)
	}
}

impl std::fmt::Display for FlvDemuxerError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Self::IO(error) => write!(f, "io error: {}", error),
			Self::Amf0Read(error) => write!(f, "amf0 read error: {}", error),
			Self::InvalidFlvHeader => write!(f, "invalid flv header"),
			Self::InvalidScriptDataName => write!(f, "invalid script data name"),
			Self::InvalidEnhancedPacketType(error) => {
				write!(f, "invalid enhanced packet type: {}", error)
			}
			Self::InvalidSoundRate(error) => {
				write!(f, "invalid sound rate: {}", error)
			}
			Self::InvalidSoundSize(error) => {
				write!(f, "invalid sound size: {}", error)
			}
			Self::InvalidSoundType(error) => {
				write!(f, "invalid sound type: {}", error)
			}
			Self::InvalidFrameType(error) => {
				write!(f, "invalid frame type: {}", error)
			}
		}
	}
}
