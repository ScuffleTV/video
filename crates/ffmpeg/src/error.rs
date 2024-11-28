use ffmpeg_sys_next::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfmpegError {
	Alloc,
	Code(FfmpegErrorCode),
	NoDecoder,
	NoEncoder,
	NoStream,
	NoFilter,
	NoFrame,
	Arguments(&'static str),
}

pub(crate) const AVERROR_EAGAIN: i32 = AVERROR(EAGAIN);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfmpegErrorCode {
	EndOfFile,
	InvalidData,
	MuxerNotFound,
	OptionNotFound,
	PatchWelcome,
	ProtocolNotFound,
	StreamNotFound,
	BitstreamFilterNotFound,
	Bug,
	BufferTooSmall,
	DecoderNotFound,
	DemuxerNotFound,
	EncoderNotFound,
	Exit,
	External,
	FilterNotFound,
	HttpBadRequest,
	HttpForbidden,
	HttpNotFound,
	HttpOther4xx,
	HttpServerError,
	HttpUnauthorized,
	Bug2,
	Unknown,
	UnknownError(i32),
}

impl std::fmt::Display for FfmpegErrorCode {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::EndOfFile => write!(f, "end of file"),
			Self::InvalidData => write!(f, "invalid data"),
			Self::MuxerNotFound => write!(f, "muxer not found"),
			Self::OptionNotFound => write!(f, "option not found"),
			Self::PatchWelcome => write!(f, "patch welcome"),
			Self::ProtocolNotFound => write!(f, "protocol not found"),
			Self::StreamNotFound => write!(f, "stream not found"),
			Self::BitstreamFilterNotFound => write!(f, "bitstream filter not found"),
			Self::Bug => write!(f, "bug"),
			Self::BufferTooSmall => write!(f, "buffer too small"),
			Self::DecoderNotFound => write!(f, "decoder not found"),
			Self::DemuxerNotFound => write!(f, "demuxer not found"),
			Self::EncoderNotFound => write!(f, "encoder not found"),
			Self::Exit => write!(f, "exit"),
			Self::External => write!(f, "external"),
			Self::FilterNotFound => write!(f, "filter not found"),
			Self::HttpBadRequest => write!(f, "http bad request"),
			Self::HttpForbidden => write!(f, "http forbidden"),
			Self::HttpNotFound => write!(f, "http not found"),
			Self::HttpOther4xx => write!(f, "http other 4xx"),
			Self::HttpServerError => write!(f, "http server error"),
			Self::HttpUnauthorized => write!(f, "http unauthorized"),
			Self::Bug2 => write!(f, "bug2"),
			Self::Unknown => write!(f, "unknown"),
			Self::UnknownError(ec) => write!(f, "unknown error code: {ec}"),
		}
	}
}

impl From<i32> for FfmpegErrorCode {
	fn from(value: i32) -> Self {
		match value {
			AVERROR_EOF => FfmpegErrorCode::EndOfFile,
			AVERROR_INVALIDDATA => FfmpegErrorCode::InvalidData,
			AVERROR_MUXER_NOT_FOUND => FfmpegErrorCode::MuxerNotFound,
			AVERROR_OPTION_NOT_FOUND => FfmpegErrorCode::OptionNotFound,
			AVERROR_PATCHWELCOME => FfmpegErrorCode::PatchWelcome,
			AVERROR_PROTOCOL_NOT_FOUND => FfmpegErrorCode::ProtocolNotFound,
			AVERROR_STREAM_NOT_FOUND => FfmpegErrorCode::StreamNotFound,
			AVERROR_BSF_NOT_FOUND => FfmpegErrorCode::BitstreamFilterNotFound,
			AVERROR_BUG => FfmpegErrorCode::Bug,
			AVERROR_BUFFER_TOO_SMALL => FfmpegErrorCode::BufferTooSmall,
			AVERROR_DECODER_NOT_FOUND => FfmpegErrorCode::DecoderNotFound,
			AVERROR_DEMUXER_NOT_FOUND => FfmpegErrorCode::DemuxerNotFound,
			AVERROR_ENCODER_NOT_FOUND => FfmpegErrorCode::EncoderNotFound,
			AVERROR_EXIT => FfmpegErrorCode::Exit,
			AVERROR_EXTERNAL => FfmpegErrorCode::External,
			AVERROR_FILTER_NOT_FOUND => FfmpegErrorCode::FilterNotFound,
			AVERROR_HTTP_BAD_REQUEST => FfmpegErrorCode::HttpBadRequest,
			AVERROR_HTTP_FORBIDDEN => FfmpegErrorCode::HttpForbidden,
			AVERROR_HTTP_NOT_FOUND => FfmpegErrorCode::HttpNotFound,
			AVERROR_HTTP_OTHER_4XX => FfmpegErrorCode::HttpOther4xx,
			AVERROR_HTTP_SERVER_ERROR => FfmpegErrorCode::HttpServerError,
			AVERROR_HTTP_UNAUTHORIZED => FfmpegErrorCode::HttpUnauthorized,
			AVERROR_BUG2 => FfmpegErrorCode::Bug2,
			AVERROR_UNKNOWN => FfmpegErrorCode::Unknown,
			_ => FfmpegErrorCode::UnknownError(value),
		}
	}
}

impl std::error::Error for FfmpegError {}

impl std::fmt::Display for FfmpegError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			FfmpegError::Alloc => write!(f, "failed to allocate memory"),
			FfmpegError::Code(code) => write!(f, "ffmpeg error: {code}"),
			FfmpegError::NoDecoder => write!(f, "no decoder found"),
			FfmpegError::NoEncoder => write!(f, "no encoder found"),
			FfmpegError::NoStream => write!(f, "no stream found"),
			FfmpegError::NoFilter => write!(f, "no filter found"),
			FfmpegError::NoFrame => write!(f, "no frame found"),
			FfmpegError::Arguments(msg) => write!(f, "invalid arguments: {}", msg),
		}
	}
}
