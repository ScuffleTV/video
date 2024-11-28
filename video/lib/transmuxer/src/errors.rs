use std::io;

#[derive(Debug)]
pub enum TransmuxError {
	InvalidVideoDimensions,
	InvalidVideoFrameRate,
	InvalidAudioSampleRate,
	InvalidHEVCDecoderConfigurationRecord,
	InvalidAv1DecoderConfigurationRecord,
	InvalidAVCDecoderConfigurationRecord,
	NoSequenceHeaders,
	IO(io::Error),
	FlvDemuxer(flv::FlvDemuxerError),
}

impl From<flv::FlvDemuxerError> for TransmuxError {
	fn from(err: flv::FlvDemuxerError) -> Self {
		Self::FlvDemuxer(err)
	}
}

impl From<io::Error> for TransmuxError {
	fn from(err: io::Error) -> Self {
		Self::IO(err)
	}
}

impl std::fmt::Display for TransmuxError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::InvalidVideoDimensions => write!(f, "invalid video dimensions"),
			Self::InvalidVideoFrameRate => write!(f, "invalid video frame rate"),
			Self::InvalidAudioSampleRate => write!(f, "invalid audio sample rate"),
			Self::InvalidHEVCDecoderConfigurationRecord => {
				write!(f, "invalid hevc decoder configuration record")
			}
			Self::InvalidAv1DecoderConfigurationRecord => {
				write!(f, "invalid av1 decoder configuration record")
			}
			Self::InvalidAVCDecoderConfigurationRecord => {
				write!(f, "invalid avc decoder configuration record")
			}
			Self::NoSequenceHeaders => write!(f, "no sequence headers"),
			Self::IO(err) => write!(f, "io error: {}", err),
			Self::FlvDemuxer(err) => write!(f, "flv demuxer error: {}", err),
		}
	}
}
