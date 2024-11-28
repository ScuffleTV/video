use std::borrow::Cow;
use std::ptr::NonNull;

use scuffle_image_processor_proto::Task;

use super::{Decoder, DecoderError, DecoderFrontend, DecoderInfo, LoopCount};
use crate::worker::process::frame::FrameRef;
use crate::worker::process::libavif::{AvifError, AvifRgbImage};
use crate::worker::process::smart_object::SmartPtr;

#[derive(Debug)]
pub struct AvifDecoder<'data> {
	decoder: SmartPtr<libavif_sys::avifDecoder>,
	info: DecoderInfo,
	_data: Cow<'data, [u8]>,
	img: AvifRgbImage,
	total_duration: u64,
	max_input_duration: Option<u64>,
}

impl<'data> AvifDecoder<'data> {
	#[tracing::instrument(skip_all, fields(name = "AvifDecoder::new"))]
	pub fn new(task: &Task, data: Cow<'data, [u8]>) -> Result<Self, DecoderError> {
		let mut decoder = SmartPtr::new(
			NonNull::new(unsafe { libavif_sys::avifDecoderCreate() }).ok_or(AvifError::OutOfMemory)?,
			|ptr| {
				// Safety: The decoder is valid.
				unsafe {
					libavif_sys::avifDecoderDestroy(ptr.as_ptr());
				}
			},
		);

		if let (Some(max_input_width), Some(max_input_height)) = (
			task.limits.as_ref().and_then(|l| l.max_input_width),
			task.limits.as_ref().and_then(|l| l.max_input_height),
		) {
			decoder.as_mut().imageDimensionLimit = max_input_width * max_input_height;
		}

		if let Some(max_input_frame_count) = task.limits.as_ref().and_then(|l| l.max_input_frame_count) {
			decoder.as_mut().imageCountLimit = max_input_frame_count;
		}

		// Safety: The decoder is valid.
		let io = NonNull::new(unsafe { libavif_sys::avifIOCreateMemoryReader(data.as_ptr(), data.len()) })
			.ok_or(AvifError::OutOfMemory)?;

		// Set the io pointer.
		decoder.as_mut().io = io.as_ptr();

		// Parse the data.
		AvifError::from_code(unsafe { libavif_sys::avifDecoderParse(decoder.as_ptr()) })?;

		let img = AvifRgbImage::new(decoder.as_ref())?;

		let info = DecoderInfo {
			width: img.width as usize,
			height: img.height as usize,
			loop_count: if decoder.as_ref().repetitionCount <= 0 {
				LoopCount::Infinite
			} else {
				LoopCount::Finite(decoder.as_ref().repetitionCount as usize)
			},
			frame_count: decoder.as_ref().imageCount.max(1) as _,
			timescale: decoder.as_ref().timescale,
		};

		if let Some(max_input_width) = task.limits.as_ref().and_then(|l| l.max_input_width) {
			if info.width > max_input_width as usize {
				return Err(DecoderError::TooWide(info.width as i32));
			}
		}

		if let Some(max_input_height) = task.limits.as_ref().and_then(|l| l.max_input_height) {
			if info.height > max_input_height as usize {
				return Err(DecoderError::TooHigh(info.height as i32));
			}
		}

		if let Some(max_input_frame_count) = task.limits.as_ref().and_then(|l| l.max_input_frame_count) {
			if info.frame_count > max_input_frame_count as usize {
				return Err(DecoderError::TooManyFrames(info.frame_count as i64));
			}
		}

		Ok(Self {
			_data: data,
			img,
			decoder,
			max_input_duration: task
				.limits
				.as_ref()
				.and_then(|l| l.max_input_duration_ms)
				.map(|max_input_duration_ms| max_input_duration_ms as u64 * info.timescale / 1000),
			total_duration: 0,
			info,
		})
	}
}

impl Decoder for AvifDecoder<'_> {
	fn backend(&self) -> DecoderFrontend {
		DecoderFrontend::LibAvif
	}

	fn info(&self) -> DecoderInfo {
		self.info
	}

	#[tracing::instrument(skip_all, fields(name = "AvifDecoder::decode"))]
	fn decode(&mut self) -> Result<Option<FrameRef>, DecoderError> {
		if AvifError::from_code(unsafe { libavif_sys::avifDecoderNextImage(self.decoder.as_ptr()) }).is_err() {
			return Ok(None);
		}

		AvifError::from_code(unsafe { libavif_sys::avifImageYUVToRGB(self.decoder.as_ref().image, &mut *self.img) })?;

		let duration_ts = self.decoder.as_ref().imageTiming.durationInTimescales;
		self.total_duration += duration_ts;

		if let Some(max_input_duration) = self.max_input_duration {
			if self.total_duration > max_input_duration {
				return Err(DecoderError::TooLong(self.total_duration as i64));
			}
		}

		Ok(Some(FrameRef::new(
			self.img.data(),
			self.img.width as usize,
			self.img.height as usize,
			duration_ts,
		)))
	}

	fn duration_ms(&self) -> i64 {
		self.total_duration as i64 * 1000 / self.info.timescale as i64
	}
}
