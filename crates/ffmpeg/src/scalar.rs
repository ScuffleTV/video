use ffmpeg_sys_next::*;

use crate::error::FfmpegError;
use crate::frame::{Frame, VideoFrame};
use crate::smart_object::SmartPtr;

pub struct Scalar {
	ptr: SmartPtr<SwsContext>,
	frame: VideoFrame,
	pixel_format: AVPixelFormat,
	width: i32,
	height: i32,
}

/// Safety: `Scalar` is safe to send between threads.
unsafe impl Send for Scalar {}

impl Scalar {
	pub fn new(
		input_width: i32,
		input_height: i32,
		incoming_pixel_fmt: AVPixelFormat,
		width: i32,
		height: i32,
		pixel_format: AVPixelFormat,
	) -> Result<Self, FfmpegError> {
		// Safety: `sws_getContext` is safe to call, and the pointer returned is valid.
		let ptr = unsafe {
			SmartPtr::wrap_non_null(
				sws_getContext(
					input_width,
					input_height,
					incoming_pixel_fmt,
					width,
					height,
					pixel_format,
					SWS_BILINEAR,
					std::ptr::null_mut(),
					std::ptr::null_mut(),
					std::ptr::null(),
				),
				|ptr| {
					sws_freeContext(*ptr);
					*ptr = std::ptr::null_mut();
				},
			)
		}
		.ok_or(FfmpegError::Alloc)?;

		let mut frame = Frame::new()?;

		unsafe {
			// Safety: `frame` is a valid pointer
			let frame_mut = frame.as_mut_ptr().as_mut().unwrap();

			frame_mut.width = width;
			frame_mut.height = height;
			frame_mut.format = pixel_format as i32;

			// Safety: `av_frame_get_buffer` is safe to call, and the pointer returned is
			// valid.
			match av_frame_get_buffer(frame_mut, 32) {
				0 => {}
				err => return Err(FfmpegError::Code(err.into())),
			}
		}

		Ok(Self {
			ptr,
			frame: frame.video(),
			pixel_format,
			width,
			height,
		})
	}

	pub fn pixel_format(&self) -> AVPixelFormat {
		self.pixel_format
	}

	pub fn width(&self) -> i32 {
		self.width
	}

	pub fn height(&self) -> i32 {
		self.height
	}

	pub fn process<'a>(&'a mut self, frame: &Frame) -> Result<&'a VideoFrame, FfmpegError> {
		// Safety: `frame` is a valid pointer, and `self.ptr` is a valid pointer.
		let ret = unsafe {
			sws_scale(
				self.ptr.as_mut_ptr(),
				frame.as_ptr().as_ref().unwrap().data.as_ptr() as *const *const u8,
				frame.as_ptr().as_ref().unwrap().linesize.as_ptr(),
				0,
				frame.as_ptr().as_ref().unwrap().height,
				self.frame.as_ptr().as_ref().unwrap().data.as_ptr(),
				self.frame.as_ptr().as_ref().unwrap().linesize.as_ptr(),
			)
		};
		if ret < 0 {
			return Err(FfmpegError::Code(ret.into()));
		}

		// Copy the other fields from the input frame to the output frame.
		self.frame.set_dts(frame.dts());
		self.frame.set_pts(frame.pts());
		self.frame.set_duration(frame.duration());
		self.frame.set_time_base(frame.time_base());

		Ok(&self.frame)
	}
}
