use ffmpeg_sys_next::*;
use libc::c_void;

use crate::error::FfmpegError;
use crate::smart_object::SmartPtr;

const AVERROR_IO: i32 = AVERROR(EIO);

/// Safety: The function must be used with the same type as the one used to
/// generically create the function pointer
pub(crate) unsafe extern "C" fn read_packet<T: std::io::Read>(
	opaque: *mut libc::c_void,
	buf: *mut u8,
	buf_size: i32,
) -> i32 {
	let ret = (*(opaque as *mut T))
		.read(std::slice::from_raw_parts_mut(buf, buf_size as usize))
		.map(|n| n as i32)
		.unwrap_or(AVERROR_IO);

	if ret == 0 {
		return AVERROR_EOF;
	}

	ret
}

/// Safety: The function must be used with the same type as the one used to
/// generically create the function pointer
pub(crate) unsafe extern "C" fn write_packet<T: std::io::Write>(
	opaque: *mut libc::c_void,
	buf: *mut u8,
	buf_size: i32,
) -> i32 {
	(*(opaque as *mut T))
		.write(std::slice::from_raw_parts(buf, buf_size as usize))
		.map(|n| n as i32)
		.unwrap_or(AVERROR_IO)
}

/// Safety: The function must be used with the same type as the one used to
/// generically create the function pointer
pub(crate) unsafe extern "C" fn seek<T: std::io::Seek>(opaque: *mut libc::c_void, offset: i64, mut whence: i32) -> i64 {
	let this = &mut *(opaque as *mut T);

	let seek_size = whence & AVSEEK_SIZE != 0;
	if seek_size {
		whence &= !AVSEEK_SIZE;
	}

	let seek_force = whence & AVSEEK_FORCE != 0;
	if seek_force {
		whence &= !AVSEEK_FORCE;
	}

	if seek_size {
		let Ok(pos) = this.stream_position() else {
			return AVERROR_IO as i64;
		};

		let Ok(end) = this.seek(std::io::SeekFrom::End(0)) else {
			return AVERROR_IO as i64;
		};

		if end != pos {
			let Ok(_) = this.seek(std::io::SeekFrom::Start(pos)) else {
				return AVERROR_IO as i64;
			};
		}

		return end as i64;
	}

	let whence = match whence {
		SEEK_SET => std::io::SeekFrom::Start(offset as u64),
		SEEK_CUR => std::io::SeekFrom::Current(offset),
		SEEK_END => std::io::SeekFrom::End(offset),
		_ => return -1,
	};

	match this.seek(whence) {
		Ok(pos) => pos as i64,
		Err(_) => AVERROR_IO as i64,
	}
}

pub(crate) struct Inner<T: Send + Sync> {
	pub(crate) data: Option<Box<T>>,
	pub(crate) context: SmartPtr<AVFormatContext>,
	_io: SmartPtr<AVIOContext>,
}

pub(crate) struct InnerOptions {
	pub(crate) buffer_size: usize,
	pub(crate) read_fn: Option<unsafe extern "C" fn(*mut c_void, *mut u8, i32) -> i32>,
	pub(crate) write_fn: Option<unsafe extern "C" fn(*mut c_void, *mut u8, i32) -> i32>,
	pub(crate) seek_fn: Option<unsafe extern "C" fn(*mut c_void, i64, i32) -> i64>,
	pub(crate) output_format: *const AVOutputFormat,
}

impl Default for InnerOptions {
	fn default() -> Self {
		Self {
			buffer_size: 4096,
			read_fn: None,
			write_fn: None,
			seek_fn: None,
			output_format: std::ptr::null(),
		}
	}
}

impl<T: Send + Sync> Inner<T> {
	pub fn new(data: T, options: InnerOptions) -> Result<Self, FfmpegError> {
		// Safety: av_malloc is safe to call
		let buffer = unsafe {
			SmartPtr::wrap_non_null(av_malloc(options.buffer_size), |ptr| {
				// We own this resource so we need to free it
				av_free(*ptr);
				// We clear the old pointer so it doesn't get freed again.
				*ptr = std::ptr::null_mut();
			})
		}
		.ok_or(FfmpegError::Alloc)?;

		let mut data = Box::new(data);

		// Safety: avio_alloc_context is safe to call, and all the function pointers are
		// valid
		let mut io = unsafe {
			SmartPtr::wrap_non_null(
				avio_alloc_context(
					buffer.as_ptr() as *mut u8,
					options.buffer_size as i32,
					if options.write_fn.is_some() { 1 } else { 0 },
					data.as_mut() as *mut _ as *mut c_void,
					options.read_fn,
					options.write_fn,
					options.seek_fn,
				),
				|ptr| {
					// Safety: the pointer is always valid.
					if let Some(ptr) = ptr.as_mut() {
						// We need to free the buffer
						av_free(ptr.buffer as *mut libc::c_void);

						// We clear the old pointer so it doesn't get freed again.
						ptr.buffer = std::ptr::null_mut();
					}

					avio_context_free(ptr);
				},
			)
		}
		.ok_or(FfmpegError::Alloc)?;

		// The buffer is now owned by the IO context
		buffer.into_inner();

		let mut context = if options.write_fn.is_some() {
			let mut context = unsafe {
				SmartPtr::wrap(std::ptr::null_mut(), |ptr| {
					// We own this resource so we need to free it
					avformat_free_context(*ptr);
					*ptr = std::ptr::null_mut();
				})
			};

			// Safety: avformat_alloc_output_context2 is safe to call
			let ec = unsafe {
				avformat_alloc_output_context2(
					context.as_mut(),
					options.output_format,
					std::ptr::null(),
					std::ptr::null_mut(),
				)
			};
			if ec != 0 {
				return Err(FfmpegError::Code(ec.into()));
			}

			if context.as_ptr().is_null() {
				return Err(FfmpegError::Alloc);
			}

			context
		} else {
			// Safety: avformat_alloc_context is safe to call
			unsafe {
				SmartPtr::wrap_non_null(avformat_alloc_context(), |ptr| {
					// We own this resource so we need to free it
					avformat_free_context(*ptr);
					*ptr = std::ptr::null_mut();
				})
			}
			.ok_or(FfmpegError::Alloc)?
		};

		// The io context will live as long as the format context
		context.as_deref_mut().expect("Context is null").pb = io.as_mut_ptr();

		Ok(Self {
			data: Some(data),
			context,
			_io: io,
		})
	}
}

impl Inner<()> {
	/// Empty context cannot be used until its initialized and setup correctly
	pub unsafe fn empty() -> Self {
		Self {
			data: Some(Box::new(())),
			context: unsafe {
				SmartPtr::wrap(std::ptr::null_mut(), |ptr| {
					// We own this resource so we need to free it
					avformat_free_context(*ptr);
					*ptr = std::ptr::null_mut();
				})
			},
			_io: unsafe { SmartPtr::wrap(std::ptr::null_mut(), |_| {}) },
		}
	}

	pub fn open_output(path: &str) -> Result<Self, FfmpegError> {
		let path = std::ffi::CString::new(path).expect("Failed to convert path to CString");

		// Safety: avformat_alloc_output_context2 is safe to call
		let mut this = unsafe { Self::empty() };

		// Safety: avformat_alloc_output_context2 is safe to call
		let ec = unsafe {
			avformat_alloc_output_context2(this.context.as_mut(), std::ptr::null(), std::ptr::null(), path.as_ptr())
		};
		if ec != 0 {
			return Err(FfmpegError::Code(ec.into()));
		}

		// We are not moving the pointer so this is safe
		if this.context.as_ptr().is_null() {
			return Err(FfmpegError::Alloc);
		}

		// Safety: avio_open is safe to call
		let ec = unsafe { avio_open(&mut this.context.as_deref_mut_except().pb, path.as_ptr(), AVIO_FLAG_WRITE) };

		if ec != 0 {
			return Err(FfmpegError::Code(ec.into()));
		}

		this.context.set_destructor(|ptr| unsafe {
			// We own this resource so we need to free it
			avio_closep(&mut (**ptr).pb);
			avformat_free_context(*ptr);
			*ptr = std::ptr::null_mut();
		});

		Ok(this)
	}
}
