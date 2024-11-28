use std::ffi::CStr;

use ffmpeg_sys_next::*;

use super::internal::{read_packet, seek, Inner, InnerOptions};
use crate::consts::{Const, DEFAULT_BUFFER_SIZE};
use crate::dict::Dictionary;
use crate::error::FfmpegError;
use crate::packet::{Packet, Packets};
use crate::smart_object::SmartObject;
use crate::stream::Streams;

pub struct Input<T: Send + Sync> {
	inner: SmartObject<Inner<T>>,
}

/// Safety: `Input` is safe to send between threads.
unsafe impl<T: Send + Sync> Send for Input<T> {}

#[derive(Debug, Clone)]
pub struct InputOptions<I: FnMut() -> bool> {
	pub buffer_size: usize,
	pub dictionary: Dictionary,
	pub interrupt_callback: Option<I>,
}

impl Default for InputOptions<fn() -> bool> {
	fn default() -> Self {
		Self {
			buffer_size: DEFAULT_BUFFER_SIZE,
			dictionary: Dictionary::new(),
			interrupt_callback: None,
		}
	}
}

impl<T: std::io::Read + Send + Sync> Input<T> {
	pub fn new(input: T) -> Result<Self, FfmpegError> {
		Self::with_options(input, &mut InputOptions::default())
	}

	pub fn with_options(input: T, options: &mut InputOptions<impl FnMut() -> bool>) -> Result<Self, FfmpegError> {
		Self::create_input(
			Inner::new(
				input,
				InnerOptions {
					buffer_size: options.buffer_size,
					read_fn: Some(read_packet::<T>),
					..Default::default()
				},
			)?,
			None,
			&mut options.dictionary,
		)
	}

	pub fn seekable(input: T) -> Result<Self, FfmpegError>
	where
		T: std::io::Seek,
	{
		Self::seekable_with_options(input, InputOptions::default())
	}

	pub fn seekable_with_options(input: T, mut options: InputOptions<impl FnMut() -> bool>) -> Result<Self, FfmpegError>
	where
		T: std::io::Seek,
	{
		Self::create_input(
			Inner::new(
				input,
				InnerOptions {
					buffer_size: options.buffer_size,
					read_fn: Some(read_packet::<T>),
					seek_fn: Some(seek::<T>),
					..Default::default()
				},
			)?,
			None,
			&mut options.dictionary,
		)
	}
}

impl<T: Send + Sync> Input<T> {
	pub fn as_ptr(&self) -> *const AVFormatContext {
		self.inner.context.as_ptr()
	}

	pub fn as_mut_ptr(&mut self) -> *mut AVFormatContext {
		self.inner.context.as_mut_ptr()
	}

	pub fn streams(&self) -> Const<'_, Streams<'_>> {
		Const::new(Streams::new(self.inner.context.as_deref_except()))
	}

	pub fn packets(&mut self) -> Packets<'_> {
		Packets::new(self.inner.context.as_deref_mut_except())
	}

	pub fn receive_packet(&mut self) -> Result<Option<Packet>, FfmpegError> {
		self.packets().receive()
	}

	fn create_input(mut inner: Inner<T>, path: Option<&CStr>, dictionary: &mut Dictionary) -> Result<Self, FfmpegError> {
		// Safety: avformat_open_input is safe to call
		let ec = unsafe {
			avformat_open_input(
				inner.context.as_mut(),
				path.map(|p| p.as_ptr()).unwrap_or(std::ptr::null()),
				std::ptr::null(),
				dictionary.as_mut_ptr_ref(),
			)
		};
		if ec != 0 {
			return Err(FfmpegError::Code(ec.into()));
		}

		if inner.context.as_ptr().is_null() {
			return Err(FfmpegError::Alloc);
		}

		let mut inner = SmartObject::new(inner, |inner| unsafe {
			// We own this resource so we need to free it
			avformat_close_input(inner.context.as_mut());
		});

		// We now own the context and this is freed when the object is dropped
		inner.context.set_destructor(|_| {});

		// Safety: avformat_find_stream_info is safe to call
		let ec = unsafe { avformat_find_stream_info(inner.context.as_mut_ptr(), std::ptr::null_mut()) };
		if ec < 0 {
			return Err(FfmpegError::Code(ec.into()));
		}

		Ok(Self { inner })
	}
}

impl Input<()> {
	pub fn open(path: &str) -> Result<Self, FfmpegError> {
		// We immediately create an input and setup the inner, before using it.
		let inner = unsafe { Inner::empty() };

		Self::create_input(inner, Some(&std::ffi::CString::new(path).unwrap()), &mut Dictionary::new())
	}
}
