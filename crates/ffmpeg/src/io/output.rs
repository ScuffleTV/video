use ffmpeg_sys_next::*;

use super::internal::{seek, write_packet, Inner, InnerOptions};
use crate::consts::DEFAULT_BUFFER_SIZE;
use crate::dict::Dictionary;
use crate::error::FfmpegError;
use crate::packet::Packet;
use crate::stream::Stream;

#[derive(Debug, Clone)]
pub struct OutputOptions<'a> {
	pub buffer_size: usize,
	pub format_name: Option<&'a str>,
	pub format_mime_type: Option<&'a str>,
	pub format_ffi: *const AVOutputFormat,
}

impl<'a> OutputOptions<'a> {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn buffer_size(mut self, buffer_size: usize) -> Self {
		self.buffer_size = buffer_size;
		self
	}

	pub fn format_name(mut self, format_name: &'a str) -> Self {
		self.format_name = format_name.into();
		self
	}

	pub fn format_mime_type(mut self, format_mime_type: &'a str) -> Self {
		self.format_mime_type = format_mime_type.into();
		self
	}

	pub fn format_ffi(mut self, format_ffi: *const AVOutputFormat) -> Self {
		self.format_ffi = format_ffi;
		self
	}

	fn get_format_ffi(&self) -> Result<*const AVOutputFormat, FfmpegError> {
		if !self.format_ffi.is_null() {
			return Ok(self.format_ffi);
		}

		if self.format_name.is_none() && self.format_mime_type.is_none() {
			return Err(FfmpegError::Arguments(
				"format_ffi, format_name and format_mime_type cannot all be unset",
			));
		}

		let c_format_name = self.format_name.map(|s| std::ffi::CString::new(s).unwrap());
		let c_format_mime_type = self.format_mime_type.map(|s| std::ffi::CString::new(s).unwrap());
		let c_format_name_ptr = c_format_name.as_ref().map(|s| s.as_ptr()).unwrap_or(std::ptr::null());
		let c_format_mime_type_ptr = c_format_mime_type.as_ref().map(|s| s.as_ptr()).unwrap_or(std::ptr::null());

		// Safety: `av_guess_format` is safe to call with null pointers.
		let output_format = unsafe { av_guess_format(c_format_name_ptr, std::ptr::null(), c_format_mime_type_ptr) };

		if output_format.is_null() {
			return Err(FfmpegError::Arguments("could not determine output format"));
		}

		Ok(output_format)
	}
}

impl Default for OutputOptions<'_> {
	fn default() -> Self {
		Self {
			buffer_size: DEFAULT_BUFFER_SIZE,
			format_name: None,
			format_mime_type: None,
			format_ffi: std::ptr::null(),
		}
	}
}

pub struct Output<T: Send + Sync> {
	inner: Inner<T>,
	witten_header: bool,
}

/// Safety: `T` must be `Send` and `Sync`.
unsafe impl<T: Send + Sync> Send for Output<T> {}

impl<T: Send + Sync> Output<T> {
	pub fn into_inner(mut self) -> T {
		*(self.inner.data.take().unwrap())
	}
}

impl<T: std::io::Write + Send + Sync> Output<T> {
	pub fn new(input: T, options: OutputOptions) -> Result<Self, FfmpegError> {
		let output_format = options.get_format_ffi()?;

		Ok(Self {
			inner: Inner::new(
				input,
				InnerOptions {
					buffer_size: options.buffer_size,
					write_fn: Some(write_packet::<T>),
					output_format,
					..Default::default()
				},
			)?,
			witten_header: false,
		})
	}

	pub fn seekable(input: T, options: OutputOptions) -> Result<Self, FfmpegError>
	where
		T: std::io::Seek,
	{
		let output_format = options.get_format_ffi()?;

		Ok(Self {
			inner: Inner::new(
				input,
				InnerOptions {
					buffer_size: options.buffer_size,
					write_fn: Some(write_packet::<T>),
					seek_fn: Some(seek::<T>),
					output_format,
					..Default::default()
				},
			)?,
			witten_header: false,
		})
	}
}

impl<T: Send + Sync> Output<T> {
	pub fn set_metadata(&mut self, metadata: Dictionary) {
		unsafe {
			// This frees the old metadata
			Dictionary::from_ptr_owned(self.inner.context.as_deref_mut_except().metadata);
			self.inner.context.as_deref_mut_except().metadata = metadata.into_ptr();
		};
	}

	pub fn as_ptr(&self) -> *const AVFormatContext {
		self.inner.context.as_ptr()
	}

	pub fn as_mut_ptr(&mut self) -> *mut AVFormatContext {
		self.inner.context.as_mut_ptr()
	}

	pub fn add_stream(&mut self, codec: Option<*const AVCodec>) -> Option<Stream<'_>> {
		// Safety: `avformat_new_stream` is safe to call.
		let stream = unsafe { avformat_new_stream(self.as_mut_ptr(), codec.unwrap_or_else(std::ptr::null)) };
		if stream.is_null() {
			None
		} else {
			// Safety: 'stream' is a valid pointer here.
			unsafe {
				let stream = &mut *stream;
				stream.id = self.inner.context.as_deref_except().nb_streams as i32 - 1;
				Some(Stream::new(stream, self.inner.context.as_deref_except()))
			}
		}
	}

	pub fn copy_stream<'a>(&'a mut self, stream: &Stream<'_>) -> Option<Stream<'a>> {
		let codec_param = stream.codec_parameters()?;

		// Safety: `avformat_new_stream` is safe to call.
		let out_stream = unsafe { avformat_new_stream(self.as_mut_ptr(), std::ptr::null()) };
		if out_stream.is_null() {
			None
		} else {
			// Safety: 'out_stream', 'codec_param' and 'context' are valid pointers here.
			unsafe {
				let out_stream = &mut *out_stream;

				// Safety: `avcodec_parameters_copy` is safe to call.
				avcodec_parameters_copy(out_stream.codecpar, codec_param);
				out_stream.id = self.inner.context.as_deref_except().nb_streams as i32 - 1;
				let mut out_stream = Stream::new(out_stream, self.inner.context.as_deref_except());
				out_stream.set_time_base(stream.time_base());
				out_stream.set_start_time(stream.start_time());
				out_stream.set_duration(stream.duration());

				Some(out_stream)
			}
		}
	}

	pub fn write_header(&mut self) -> Result<(), FfmpegError> {
		if self.witten_header {
			return Err(FfmpegError::Arguments("header already written"));
		}

		// Safety: `avformat_write_header` is safe to call, if the header has not been
		// written yet.
		unsafe {
			match avformat_write_header(self.as_mut_ptr(), std::ptr::null_mut()) {
				0 => Ok(()),
				e => Err(FfmpegError::Code(e.into())),
			}
		}?;

		self.witten_header = true;
		Ok(())
	}

	pub fn write_header_with_options(&mut self, options: &mut Dictionary) -> Result<(), FfmpegError> {
		if self.witten_header {
			return Err(FfmpegError::Arguments("header already written"));
		}

		// Safety: `avformat_write_header` is safe to call, if the header has not been
		// written yet.
		unsafe {
			match avformat_write_header(self.as_mut_ptr(), options.as_mut_ptr_ref()) {
				0 => Ok(()),
				e => Err(FfmpegError::Code(e.into())),
			}
		}?;

		self.witten_header = true;
		Ok(())
	}

	pub fn write_trailer(&mut self) -> Result<(), FfmpegError> {
		if !self.witten_header {
			return Err(FfmpegError::Arguments("header not written"));
		}

		// Safety: `av_write_trailer` is safe to call, once the header has been written.
		unsafe {
			match av_write_trailer(self.as_mut_ptr()) {
				n if n >= 0 => Ok(()),
				e => Err(FfmpegError::Code(e.into())),
			}
		}
	}

	pub fn write_interleaved_packet(&mut self, mut packet: Packet) -> Result<(), FfmpegError> {
		if !self.witten_header {
			return Err(FfmpegError::Arguments("header not written"));
		}

		// Safety: `av_interleaved_write_frame` is safe to call, once the header has
		// been written.
		unsafe {
			match av_interleaved_write_frame(self.as_mut_ptr(), packet.as_mut_ptr()) {
				0 => Ok(()),
				e => Err(FfmpegError::Code(e.into())),
			}
		}
	}

	pub fn write_packet(&mut self, packet: &Packet) -> Result<(), FfmpegError> {
		if !self.witten_header {
			return Err(FfmpegError::Arguments("header not written"));
		}

		// Safety: `av_write_frame` is safe to call, once the header has been written.
		unsafe {
			match av_write_frame(self.as_mut_ptr(), packet.as_ptr() as *mut _) {
				0 => Ok(()),
				e => Err(FfmpegError::Code(e.into())),
			}
		}
	}

	pub fn flags(&self) -> i32 {
		self.inner.context.as_deref_except().flags
	}
}

impl Output<()> {
	pub fn open(path: &str) -> Result<Self, FfmpegError> {
		Ok(Self {
			inner: Inner::open_output(path)?,
			witten_header: false,
		})
	}
}
