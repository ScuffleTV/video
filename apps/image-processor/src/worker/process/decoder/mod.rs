use std::borrow::Cow;

use file_format::FileFormat;
use scuffle_ffmpeg::error::FfmpegError;
use scuffle_image_processor_proto::Task;

use super::frame::FrameRef;
use super::libavif::AvifError;
use super::libwebp::WebPError;

mod ffmpeg;
mod libavif;
mod libwebp;

#[derive(Debug, thiserror::Error)]
pub enum DecoderError {
	#[error("ffmpeg: {0}")]
	Ffmpeg(#[from] FfmpegError),
	#[error("libavif: {0}")]
	LibAvif(#[from] AvifError),
	#[error("libwebp: {0}")]
	LibWebp(#[from] WebPError),
	#[error("unsupported input format: {0}")]
	UnsupportedInputFormat(FileFormat),
	#[error("no video stream")]
	NoVideoStream,
	#[error("no frame count")]
	NoFrameCount,
	#[error("invalid time base")]
	InvalidTimeBase,
	#[error("invalid video decoder")]
	InvalidVideoDecoder,
	#[error("exceeded maximum input width: {0}")]
	TooWide(i32),
	#[error("exceeded maximum input height: {0}")]
	TooHigh(i32),
	#[error("exceeded maximum input frame count: {0}")]
	TooManyFrames(i64),
	#[error("exceeded maximum input duration: {0}")]
	TooLong(i64),
	#[error("malformed input")]
	MalformedInput,
}

#[derive(Debug, Clone, Copy)]
pub enum DecoderFrontend {
	Ffmpeg,
	LibWebp,
	LibAvif,
}

impl DecoderFrontend {
	pub const fn from_format(format: FileFormat) -> Result<Self, DecoderError> {
		match format {
			FileFormat::Webp => Ok(Self::LibWebp), // .webp
			FileFormat::Av1ImageFileFormat  // .avif
			| FileFormat::Av1ImageFileFormatSequence => Ok(Self::LibAvif), // .avifs
			FileFormat::GraphicsInterchangeFormat // .gif
			| FileFormat::PortableNetworkGraphics // .png
			| FileFormat::AnimatedPortableNetworkGraphics // .apng
			| FileFormat::JpegLs // .jls
			| FileFormat::JointPhotographicExpertsGroup // .jpg
			| FileFormat::JpegXl // .jxl
			| FileFormat::WindowsBitmap // .bmp
			| FileFormat::HighEfficiencyImageCoding // .heic
			| FileFormat::HighEfficiencyImageCodingSequence // .heics
			| FileFormat::HighEfficiencyImageFileFormat // .heif
			| FileFormat::HighEfficiencyImageFileFormatSequence // .heifs
			| FileFormat::Mpeg4Part14 // .mp4
			| FileFormat::Mpeg4Part14Video // .mp4v
			| FileFormat::FlashVideo // .flv
			| FileFormat::Matroska3dVideo // .mk3d
			| FileFormat::MatroskaVideo // .mkv
			| FileFormat::AudioVideoInterleave // .avi
			| FileFormat::AppleQuicktime // .mov
			| FileFormat::Webm // .webm
			| FileFormat::BdavMpeg2TransportStream // .m2ts
			| FileFormat::Mpeg2TransportStream => Ok(Self::Ffmpeg), // .ts
			_ => Err(DecoderError::UnsupportedInputFormat(format)),
		}
	}

	pub fn build<'a>(&self, task: &Task, data: Cow<'a, [u8]>) -> Result<AnyDecoder<'a>, DecoderError> {
		match self {
			Self::Ffmpeg => Ok(AnyDecoder::Ffmpeg(ffmpeg::FfmpegDecoder::new(task, data)?)),
			Self::LibAvif => Ok(AnyDecoder::LibAvif(libavif::AvifDecoder::new(task, data)?)),
			Self::LibWebp => Ok(AnyDecoder::LibWebp(libwebp::WebpDecoder::new(task, data)?)),
		}
	}
}

pub enum AnyDecoder<'a> {
	Ffmpeg(ffmpeg::FfmpegDecoder<'a>),
	LibAvif(libavif::AvifDecoder<'a>),
	LibWebp(libwebp::WebpDecoder<'a>),
}

pub trait Decoder {
	fn backend(&self) -> DecoderFrontend;
	fn info(&self) -> DecoderInfo;
	fn duration_ms(&self) -> i64;
	fn decode(&mut self) -> Result<Option<FrameRef>, DecoderError>;
}

#[derive(Debug, Clone, Copy)]
pub struct DecoderInfo {
	pub width: usize,
	pub height: usize,
	pub loop_count: LoopCount,
	pub frame_count: usize,
	pub timescale: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopCount {
	Infinite,
	Finite(usize),
}

impl LoopCount {
	pub fn as_i32(self) -> i32 {
		match self {
			LoopCount::Infinite => -1,
			LoopCount::Finite(count) => count as i32,
		}
	}
}

impl Decoder for AnyDecoder<'_> {
	fn backend(&self) -> DecoderFrontend {
		match self {
			Self::Ffmpeg(decoder) => decoder.backend(),
			Self::LibAvif(decoder) => decoder.backend(),
			Self::LibWebp(decoder) => decoder.backend(),
		}
	}

	fn info(&self) -> DecoderInfo {
		match self {
			Self::Ffmpeg(decoder) => decoder.info(),
			Self::LibAvif(decoder) => decoder.info(),
			Self::LibWebp(decoder) => decoder.info(),
		}
	}

	fn decode(&mut self) -> Result<Option<FrameRef>, DecoderError> {
		match self {
			Self::Ffmpeg(decoder) => decoder.decode(),
			Self::LibAvif(decoder) => decoder.decode(),
			Self::LibWebp(decoder) => decoder.decode(),
		}
	}

	fn duration_ms(&self) -> i64 {
		match self {
			Self::Ffmpeg(decoder) => decoder.duration_ms(),
			Self::LibAvif(decoder) => decoder.duration_ms(),
			Self::LibWebp(decoder) => decoder.duration_ms(),
		}
	}
}
