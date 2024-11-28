use std::borrow::Cow;
use std::ptr::NonNull;

use scuffle_image_processor_proto::Task;

use super::{Decoder, DecoderError, DecoderFrontend, DecoderInfo, LoopCount};
use crate::worker::process::frame::FrameRef;
use crate::worker::process::libwebp::{zero_memory_default, WebPError};
use crate::worker::process::smart_object::SmartPtr;

pub struct WebpDecoder<'data> {
	info: DecoderInfo,
	decoder: SmartPtr<libwebp_sys::WebPAnimDecoder>,
	_data: Cow<'data, [u8]>,
	timestamp: i32,
	total_duration: u64,
	max_input_duration: Option<u64>,
}

impl<'data> WebpDecoder<'data> {
	#[tracing::instrument(skip_all, fields(name = "WebpDecoder::new"))]
	pub fn new(task: &Task, data: Cow<'data, [u8]>) -> Result<Self, DecoderError> {
		let decoder = SmartPtr::new(
			NonNull::new(unsafe {
				libwebp_sys::WebPAnimDecoderNew(
					&libwebp_sys::WebPData {
						bytes: data.as_ptr(),
						size: data.len(),
					},
					std::ptr::null(),
				)
			})
			.ok_or(WebPError::OutOfMemory)?,
			|decoder| {
				// Safety: The decoder is valid.
				unsafe {
					libwebp_sys::WebPAnimDecoderDelete(decoder.as_ptr());
				}
			},
		);

		let mut info = zero_memory_default::<libwebp_sys::WebPAnimInfo>();

		// Safety: both pointers are valid and the decoder is valid.
		if unsafe { libwebp_sys::WebPAnimDecoderGetInfo(decoder.as_ptr(), &mut info) } == 0 {
			return Err(DecoderError::LibWebp(WebPError::InvalidData));
		}

		if let Some(max_input_width) = task.limits.as_ref().and_then(|l| l.max_input_width) {
			if info.canvas_width > max_input_width {
				return Err(DecoderError::TooWide(info.canvas_width as i32));
			}
		}

		if let Some(max_input_height) = task.limits.as_ref().and_then(|l| l.max_input_height) {
			if info.canvas_height > max_input_height {
				return Err(DecoderError::TooHigh(info.canvas_height as i32));
			}
		}

		if let Some(max_input_frame_count) = task.limits.as_ref().and_then(|l| l.max_input_frame_count) {
			if info.frame_count > max_input_frame_count {
				return Err(DecoderError::TooManyFrames(info.frame_count as i64));
			}
		}

		Ok(Self {
			info: DecoderInfo {
				width: info.canvas_width as usize,
				height: info.canvas_height as usize,
				loop_count: match info.loop_count {
					0 => LoopCount::Infinite,
					_ => LoopCount::Finite(info.loop_count as usize),
				},
				frame_count: info.frame_count as usize,
				timescale: 1000,
			},
			max_input_duration: task
				.limits
				.as_ref()
				.and_then(|l| l.max_input_duration_ms)
				.map(|dur| dur as u64),
			decoder,
			_data: data,
			total_duration: 0,
			timestamp: 0,
		})
	}
}

impl Decoder for WebpDecoder<'_> {
	fn backend(&self) -> DecoderFrontend {
		DecoderFrontend::LibWebp
	}

	fn info(&self) -> DecoderInfo {
		self.info
	}

	#[tracing::instrument(skip_all, fields(name = "WebpDecoder::decode"))]
	fn decode(&mut self) -> Result<Option<FrameRef>, DecoderError> {
		let mut buf = std::ptr::null_mut();
		let previous_timestamp = self.timestamp;

		// Safety: The buffer is a valid pointer to a null ptr, timestamp is a valid
		// pointer to i32, and the decoder is valid.
		let result = unsafe { libwebp_sys::WebPAnimDecoderGetNext(self.decoder.as_ptr(), &mut buf, &mut self.timestamp) };

		// If 0 is returned, the animation is over.
		if result == 0 {
			return Ok(None);
		}

		let buf = NonNull::new(buf).ok_or(WebPError::OutOfMemory)?;

		let buf =
			unsafe { std::slice::from_raw_parts(buf.as_ptr() as *const rgb::RGBA8, self.info.width * self.info.height) };

		let duration_ts = (self.timestamp - previous_timestamp).max(0) as u64;
		self.total_duration += duration_ts;

		if let Some(max_input_duration) = self.max_input_duration {
			if self.total_duration > max_input_duration {
				return Err(DecoderError::TooLong(self.total_duration as i64));
			}
		}

		let duration_ts = (self.timestamp - previous_timestamp).max(0) as u64;

		Ok(Some(FrameRef::new(buf, self.info.width, self.info.height, duration_ts)))
	}

	fn duration_ms(&self) -> i64 {
		self.total_duration as i64
	}
}
