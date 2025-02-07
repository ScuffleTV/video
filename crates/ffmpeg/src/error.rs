use ffmpeg_sys_next::*;
use nutype_enum::nutype_enum;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
/// An error that occurs when the ffmpeg operation fails.
pub enum FfmpegError {
    /// An error that occurs when the memory allocation fails.
    #[error("failed to allocate memory")]
    Alloc,
    /// An error that occurs when the ffmpeg error code is not a success code.
    #[error("ffmpeg error: {0}")]
    Code(#[from] FfmpegErrorCode),
    /// An error that occurs when no decoder is found.
    #[error("no decoder found")]
    NoDecoder,
    /// An error that occurs when no encoder is found.
    #[error("no encoder found")]
    NoEncoder,
    /// An error that occurs when no stream is found.
    #[error("no stream found")]
    NoStream,
    /// An error that occurs when no filter is found.
    #[error("no filter found")]
    NoFilter,
    /// An error that occurs when no frame is found.
    #[error("no frame found")]
    NoFrame,
    /// An error that occurs when the arguments are invalid.
    #[error("invalid arguments: {0}")]
    Arguments(&'static str),
}

pub(crate) const AVERROR_EAGAIN: i32 = AVERROR(EAGAIN);

nutype_enum! {
    /// An enum that represents the ffmpeg error code.
    pub enum FfmpegErrorCode(i32) {
        /// FFmpeg error code for end of file.
        EndOfFile = AVERROR_EOF,
        /// FFmpeg error code for invalid data.
        InvalidData = AVERROR_INVALIDDATA,
        /// FFmpeg error code for muxer not found.
        MuxerNotFound = AVERROR_MUXER_NOT_FOUND,
        /// FFmpeg error code for option not found.
        OptionNotFound = AVERROR_OPTION_NOT_FOUND,
        /// FFmpeg error code for patch welcome.
        PatchWelcome = AVERROR_PATCHWELCOME,
        /// FFmpeg error code for protocol not found.
        ProtocolNotFound = AVERROR_PROTOCOL_NOT_FOUND,
        /// FFmpeg error code for stream not found.
        StreamNotFound = AVERROR_STREAM_NOT_FOUND,
        /// FFmpeg error code for bitstream filter not found.
        BitstreamFilterNotFound = AVERROR_BSF_NOT_FOUND,
        /// FFmpeg error code for bug.
        Bug = AVERROR_BUG,
        /// FFmpeg error code for eof.
        Eof = AVERROR_EOF,
        /// FFmpeg error code for eagain.
        Eagain = AVERROR_EAGAIN,
        /// FFmpeg error code for buffer too small.
        BufferTooSmall = AVERROR_BUFFER_TOO_SMALL,
        /// FFmpeg error code for decoder not found.
        DecoderNotFound = AVERROR_DECODER_NOT_FOUND,
        /// FFmpeg error code for demuxer not found.
        DemuxerNotFound = AVERROR_DEMUXER_NOT_FOUND,
        /// FFmpeg error code for encoder not found.
        EncoderNotFound = AVERROR_ENCODER_NOT_FOUND,
        /// FFmpeg error code for exit.
        Exit = AVERROR_EXIT,
        /// FFmpeg error code for external.
        External = AVERROR_EXTERNAL,
        /// FFmpeg error code for filter not found.
        FilterNotFound = AVERROR_FILTER_NOT_FOUND,
        /// FFmpeg error code for http bad request.
        HttpBadRequest = AVERROR_HTTP_BAD_REQUEST,
        /// FFmpeg error code for http forbidden.
        HttpForbidden = AVERROR_HTTP_FORBIDDEN,
        /// FFmpeg error code for http not found.
        HttpNotFound = AVERROR_HTTP_NOT_FOUND,
        /// FFmpeg error code for http other 4xx.
        HttpOther4xx = AVERROR_HTTP_OTHER_4XX,
        /// FFmpeg error code for http server error.
        HttpServerError = AVERROR_HTTP_SERVER_ERROR,
        /// FFmpeg error code for http unauthorized.
        HttpUnauthorized = AVERROR_HTTP_UNAUTHORIZED,
        /// FFmpeg error code for bug2.
        Bug2 = AVERROR_BUG2,
        /// FFmpeg error code for unknown.
        Unknown = AVERROR_UNKNOWN,
    }
}

impl FfmpegErrorCode {
    /// Returns the result of the error code.
    pub const fn result(self) -> Result<i32, FfmpegError> {
        match self {
            code if code.is_success() => Ok(code.0),
            _ => Err(FfmpegError::Code(self)),
        }
    }

    /// Returns true if the error code is a success code.
    pub const fn is_success(self) -> bool {
        self.0 >= 0
    }
}

impl std::fmt::Display for FfmpegErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
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
            Self(ec) => write!(f, "unknown error code: {ec}"),
        }
    }
}

impl std::error::Error for FfmpegErrorCode {}

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use super::{FfmpegError, FfmpegErrorCode};
    use crate::error::*;

    #[test]
    fn test_ffmpeg_error_code_display() {
        let cases = [
            (FfmpegErrorCode::EndOfFile, "end of file"),
            (FfmpegErrorCode::InvalidData, "invalid data"),
            (FfmpegErrorCode::MuxerNotFound, "muxer not found"),
            (FfmpegErrorCode::OptionNotFound, "option not found"),
            (FfmpegErrorCode::PatchWelcome, "patch welcome"),
            (FfmpegErrorCode::ProtocolNotFound, "protocol not found"),
            (FfmpegErrorCode::StreamNotFound, "stream not found"),
            (FfmpegErrorCode::BitstreamFilterNotFound, "bitstream filter not found"),
            (FfmpegErrorCode::Bug, "bug"),
            (FfmpegErrorCode::BufferTooSmall, "buffer too small"),
            (FfmpegErrorCode::DecoderNotFound, "decoder not found"),
            (FfmpegErrorCode::DemuxerNotFound, "demuxer not found"),
            (FfmpegErrorCode::EncoderNotFound, "encoder not found"),
            (FfmpegErrorCode::Exit, "exit"),
            (FfmpegErrorCode::External, "external"),
            (FfmpegErrorCode::FilterNotFound, "filter not found"),
            (FfmpegErrorCode::HttpBadRequest, "http bad request"),
            (FfmpegErrorCode::HttpForbidden, "http forbidden"),
            (FfmpegErrorCode::HttpNotFound, "http not found"),
            (FfmpegErrorCode::HttpOther4xx, "http other 4xx"),
            (FfmpegErrorCode::HttpServerError, "http server error"),
            (FfmpegErrorCode::HttpUnauthorized, "http unauthorized"),
            (FfmpegErrorCode::Bug2, "bug2"),
            (FfmpegErrorCode::Unknown, "unknown"),
            (FfmpegErrorCode(123), "unknown error code: 123"),
        ];

        for (code, expected) in cases {
            assert_eq!(code.to_string(), expected);
        }
    }

    #[test]
    fn test_ffmpeg_error_code_from_i32() {
        // Define constants that map to the FfmpegErrorCode variants
        const TEST_CASES: &[(i32, FfmpegErrorCode)] = &[
            (AVERROR_EOF, FfmpegErrorCode::EndOfFile),
            (AVERROR_INVALIDDATA, FfmpegErrorCode::InvalidData),
            (AVERROR_MUXER_NOT_FOUND, FfmpegErrorCode::MuxerNotFound),
            (AVERROR_OPTION_NOT_FOUND, FfmpegErrorCode::OptionNotFound),
            (AVERROR_PATCHWELCOME, FfmpegErrorCode::PatchWelcome),
            (AVERROR_PROTOCOL_NOT_FOUND, FfmpegErrorCode::ProtocolNotFound),
            (AVERROR_STREAM_NOT_FOUND, FfmpegErrorCode::StreamNotFound),
            (AVERROR_BSF_NOT_FOUND, FfmpegErrorCode::BitstreamFilterNotFound),
            (AVERROR_BUG, FfmpegErrorCode::Bug),
            (AVERROR_BUFFER_TOO_SMALL, FfmpegErrorCode::BufferTooSmall),
            (AVERROR_DECODER_NOT_FOUND, FfmpegErrorCode::DecoderNotFound),
            (AVERROR_DEMUXER_NOT_FOUND, FfmpegErrorCode::DemuxerNotFound),
            (AVERROR_ENCODER_NOT_FOUND, FfmpegErrorCode::EncoderNotFound),
            (AVERROR_EXIT, FfmpegErrorCode::Exit),
            (AVERROR_EXTERNAL, FfmpegErrorCode::External),
            (AVERROR_FILTER_NOT_FOUND, FfmpegErrorCode::FilterNotFound),
            (AVERROR_HTTP_BAD_REQUEST, FfmpegErrorCode::HttpBadRequest),
            (AVERROR_HTTP_FORBIDDEN, FfmpegErrorCode::HttpForbidden),
            (AVERROR_HTTP_NOT_FOUND, FfmpegErrorCode::HttpNotFound),
            (AVERROR_HTTP_OTHER_4XX, FfmpegErrorCode::HttpOther4xx),
            (AVERROR_HTTP_SERVER_ERROR, FfmpegErrorCode::HttpServerError),
            (AVERROR_HTTP_UNAUTHORIZED, FfmpegErrorCode::HttpUnauthorized),
            (AVERROR_BUG2, FfmpegErrorCode::Bug2),
            (AVERROR_UNKNOWN, FfmpegErrorCode::Unknown),
        ];

        // Test each case
        for &(value, expected) in TEST_CASES {
            let result: FfmpegErrorCode = value.into();
            assert_eq!(result, expected, "Failed for value: {value}");
        }

        // Test an unknown error case
        let unknown_value = 9999;
        let result: FfmpegErrorCode = unknown_value.into();
        assert_eq!(
            result,
            FfmpegErrorCode(unknown_value),
            "Failed for unknown value: {unknown_value}"
        );
    }

    #[test]
    fn test_ffmpeg_error_display() {
        let cases = [
            (FfmpegError::Alloc, "failed to allocate memory"),
            (FfmpegError::Code(FfmpegErrorCode::EndOfFile), "ffmpeg error: end of file"),
            (FfmpegError::NoDecoder, "no decoder found"),
            (FfmpegError::NoEncoder, "no encoder found"),
            (FfmpegError::NoStream, "no stream found"),
            (FfmpegError::NoFilter, "no filter found"),
            (FfmpegError::NoFrame, "no frame found"),
            (
                FfmpegError::Arguments("invalid argument example"),
                "invalid arguments: invalid argument example",
            ),
        ];

        for (error, expected) in cases {
            assert_eq!(error.to_string(), expected);
        }
    }
}
