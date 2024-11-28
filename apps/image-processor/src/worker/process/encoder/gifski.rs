use scuffle_image_processor_proto::OutputQuality;

use super::{Encoder, EncoderError, EncoderInfo, EncoderSettings};
use crate::worker::process::decoder::LoopCount;
use crate::worker::process::frame::FrameRef;

pub struct GifskiEncoder {
	collector: gifski::Collector,
	writer: std::thread::JoinHandle<Result<Vec<u8>, EncoderError>>,
	info: EncoderInfo,
}

impl GifskiEncoder {
	#[tracing::instrument(skip_all, fields(name = "GifskiEncoder::new"))]
	pub fn new(settings: EncoderSettings) -> Result<Self, EncoderError> {
		let (collector, writer) = gifski::new(gifski::Settings {
			repeat: match settings.loop_count {
				LoopCount::Infinite => gifski::Repeat::Infinite,
				LoopCount::Finite(count) => gifski::Repeat::Finite(count as u16),
			},
			quality: match settings.quality {
				OutputQuality::Auto => 100,
				OutputQuality::High => 100,
				OutputQuality::Lossless => 100,
				OutputQuality::Medium => 75,
				OutputQuality::Low => 50,
			},
			fast: match settings.quality {
				OutputQuality::Auto => true,
				OutputQuality::High => false,
				OutputQuality::Lossless => false,
				OutputQuality::Medium => true,
				OutputQuality::Low => true,
			},
			..Default::default()
		})?;

		Ok(Self {
			collector,
			writer: std::thread::spawn(move || {
				let mut buffer = Vec::new();
				writer.write(&mut buffer, &mut gifski::progress::NoProgress {})?;
				Ok(buffer)
			}),
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

	fn duration(&mut self, duration: u64) -> f64 {
		self.info.duration += duration;
		self.info.duration as f64 / self.info.timescale as f64
	}
}

impl Encoder for GifskiEncoder {
	fn info(&self) -> &EncoderInfo {
		&self.info
	}

	#[tracing::instrument(skip_all, fields(name = "GifskiEncoder::add_frame"))]
	fn add_frame(&mut self, frame: FrameRef) -> Result<(), EncoderError> {
		let frame = frame.to_owned();
		self.info.height = frame.image.height();
		self.info.width = frame.image.width();
		let duration = self.duration(frame.duration_ts);
		self.collector.add_frame_rgba(self.info.frame_count, frame.image, duration)?;
		self.info.frame_count += 1;
		Ok(())
	}

	#[tracing::instrument(skip_all, fields(name = "GifskiEncoder::finish"))]
	fn finish(self) -> Result<Vec<u8>, EncoderError> {
		drop(self.collector);

		self.writer.join().map_err(|_| EncoderError::Thread)?
	}
}
