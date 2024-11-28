use scuffle_image_processor_proto::{OutputFormat, OutputQuality};

use super::decoder::LoopCount;
use super::frame::FrameRef;
use super::libavif::AvifError;
use super::libwebp::WebPError;

mod gifski;
mod libavif;
mod libwebp;
mod png;

#[derive(Debug, Clone, Copy)]
pub enum EncoderBackend {
	Gifski,
	Png,
	LibWebp,
	LibAvif,
}

#[derive(Debug, Clone)]
pub struct EncoderSettings {
	pub name: Option<String>,
	pub format: OutputFormat,
	pub quality: OutputQuality,
	pub loop_count: LoopCount,
	pub timescale: u64,
	pub static_image: bool,
}

#[derive(Debug, Clone)]
pub struct EncoderInfo {
	pub name: Option<String>,
	pub format: OutputFormat,
	pub width: usize,
	pub height: usize,
	pub timescale: u64,
	pub duration: u64,
	pub frame_count: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum EncoderError {
	#[error("gifski: {0}")]
	Gifski(#[from] ::gifski::Error),
	#[error("thread panicked")]
	Thread,
	#[error("avif: {0}")]
	Avif(#[from] AvifError),
	#[error("no frames added")]
	NoFrames,
	#[error("static image has multiple frames")]
	MultipleFrames,
	#[error("webp: {0}")]
	Webp(#[from] WebPError),
	#[error("png: {0}")]
	Png(#[from] ::png::EncodingError),
}

impl EncoderBackend {
	pub fn build(&self, settings: EncoderSettings) -> Result<AnyEncoder, EncoderError> {
		match self {
			Self::Png => Ok(AnyEncoder::Png(png::PngEncoder::new(settings)?)),
			Self::Gifski => Ok(AnyEncoder::Gifski(gifski::GifskiEncoder::new(settings)?)),
			Self::LibAvif => Ok(AnyEncoder::LibAvif(libavif::AvifEncoder::new(settings)?)),
			Self::LibWebp => Ok(AnyEncoder::LibWebp(libwebp::WebpEncoder::new(settings)?)),
		}
	}
}

pub enum AnyEncoder {
	Gifski(gifski::GifskiEncoder),
	Png(png::PngEncoder),
	LibAvif(libavif::AvifEncoder),
	LibWebp(libwebp::WebpEncoder),
}

pub trait Encoder {
	fn info(&self) -> &EncoderInfo;
	fn add_frame(&mut self, frame: FrameRef) -> Result<(), EncoderError>;
	fn finish(self) -> Result<Vec<u8>, EncoderError>;
}

impl Encoder for AnyEncoder {
	fn info(&self) -> &EncoderInfo {
		match self {
			Self::Gifski(encoder) => encoder.info(),
			Self::Png(encoder) => encoder.info(),
			Self::LibAvif(encoder) => encoder.info(),
			Self::LibWebp(encoder) => encoder.info(),
		}
	}

	fn add_frame(&mut self, frame: FrameRef) -> Result<(), EncoderError> {
		match self {
			Self::Gifski(encoder) => encoder.add_frame(frame),
			Self::Png(encoder) => encoder.add_frame(frame),
			Self::LibAvif(encoder) => encoder.add_frame(frame),
			Self::LibWebp(encoder) => encoder.add_frame(frame),
		}
	}

	fn finish(self) -> Result<Vec<u8>, EncoderError> {
		match self {
			Self::Gifski(encoder) => encoder.finish(),
			Self::Png(encoder) => encoder.finish(),
			Self::LibAvif(encoder) => encoder.finish(),
			Self::LibWebp(encoder) => encoder.finish(),
		}
	}
}
