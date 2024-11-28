use rgb::ComponentBytes;

use super::{Encoder, EncoderError, EncoderInfo, EncoderSettings};
use crate::worker::process::frame::FrameRef;

pub struct PngEncoder {
	result: Option<Vec<u8>>,
	info: EncoderInfo,
}

impl PngEncoder {
	#[tracing::instrument(skip_all, fields(name = "PngEncoder::new"))]
	pub fn new(settings: EncoderSettings) -> Result<Self, EncoderError> {
		Ok(Self {
			result: None,
			info: EncoderInfo {
				name: settings.name,
				duration: 0,
				frame_count: 0,
				format: settings.format,
				height: 0,
				timescale: settings.timescale,
				width: 0,
			},
		})
	}
}

impl Encoder for PngEncoder {
	fn info(&self) -> &EncoderInfo {
		&self.info
	}

	#[tracing::instrument(skip_all, fields(name = "PngEncoder::add_frame"))]
	fn add_frame(&mut self, frame: FrameRef) -> Result<(), EncoderError> {
		if self.result.is_some() {
			return Err(EncoderError::MultipleFrames);
		}

		self.info.height = frame.image.height();
		self.info.width = frame.image.width();
		self.info.frame_count += 1;

		let mut result = Vec::new();

		let mut encoder = png::Encoder::new(&mut result, frame.image.width() as u32, frame.image.height() as u32);

		assert!(
			frame.image.buf().as_bytes().len() == frame.image.width() * frame.image.height() * 4,
			"image buffer size mismatch"
		);

		encoder.set_color(png::ColorType::Rgba);
		encoder.set_depth(png::BitDepth::Eight);
		encoder.write_header()?.write_image_data(frame.image.buf().as_bytes())?;

		self.result = Some(result);

		Ok(())
	}

	#[tracing::instrument(skip_all, fields(name = "PngEncoder::finish"))]
	fn finish(self) -> Result<Vec<u8>, EncoderError> {
		self.result.ok_or(EncoderError::NoFrames)
	}
}
