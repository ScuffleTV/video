use std::ptr::NonNull;

use scuffle_image_processor_proto::OutputQuality;

use super::{Encoder, EncoderError, EncoderInfo, EncoderSettings};
use crate::worker::process::decoder::LoopCount;
use crate::worker::process::frame::FrameRef;
use crate::worker::process::libwebp::{zero_memory_default, WebPError};
use crate::worker::process::smart_object::{SmartObject, SmartPtr};

pub struct WebpEncoder {
	config: libwebp_sys::WebPConfig,
	settings: EncoderSettings,
	picture: SmartObject<libwebp_sys::WebPPicture>,
	encoder: Option<SmartPtr<libwebp_sys::WebPAnimEncoder>>,
	first_duration: Option<u64>,
	info: EncoderInfo,
	static_image: bool,
}

fn wrap_error(status: i32, err: &'static str) -> Result<(), WebPError> {
	if status == 0 {
		Err(WebPError::UnknownError(err))
	} else {
		Ok(())
	}
}

impl WebpEncoder {
	#[tracing::instrument(skip_all, fields(name = "WebpEncoder::new"))]
	pub fn new(settings: EncoderSettings) -> Result<Self, EncoderError> {
		let mut config = zero_memory_default::<libwebp_sys::WebPConfig>();

		wrap_error(
			unsafe { libwebp_sys::WebPConfigInit(&mut config) },
			"failed to initialize webp config",
		)?;

		config.lossless = if settings.quality == OutputQuality::Lossless { 1 } else { 0 };
		config.quality = match settings.quality {
			OutputQuality::Auto => 75.0,
			OutputQuality::High => 95.0,
			OutputQuality::Lossless => 100.0,
			OutputQuality::Medium => 75.0,
			OutputQuality::Low => 50.0,
		};
		config.alpha_quality = 100;
		config.method = match settings.quality {
			OutputQuality::Auto => 4,
			OutputQuality::High => 5,
			OutputQuality::Lossless => 6,
			OutputQuality::Medium => 3,
			OutputQuality::Low => 2,
		};
		config.thread_level = 1;
		config.qmin = 75;
		config.qmax = 100;

		let mut picture = SmartObject::new(zero_memory_default::<libwebp_sys::WebPPicture>(), |ptr| unsafe {
			libwebp_sys::WebPPictureFree(ptr);
		});

		wrap_error(
			unsafe { libwebp_sys::WebPPictureInit(&mut *picture) },
			"failed to initialize webp picture",
		)?;

		picture.use_argb = 1;

		Ok(Self {
			config,
			info: EncoderInfo {
				name: settings.name.clone(),
				duration: 0,
				frame_count: 0,
				format: settings.format,
				height: 0,
				timescale: settings.timescale,
				width: 0,
			},
			static_image: settings.static_image,
			settings,
			picture,
			encoder: None,
			first_duration: None,
		})
	}

	fn timestamp(&self) -> u64 {
		self.info.duration * 1000 / self.settings.timescale
	}

	fn flush_frame(&mut self, duration: u64) -> Result<(), EncoderError> {
		// Safety: The picture is valid.
		wrap_error(
			unsafe {
				libwebp_sys::WebPAnimEncoderAdd(
					self.encoder.as_mut().unwrap().as_ptr(),
					&mut *self.picture,
					self.timestamp() as _,
					&self.config,
				)
			},
			"failed to add webp frame",
		)?;

		self.info.duration += duration;

		Ok(())
	}
}

impl Encoder for WebpEncoder {
	fn info(&self) -> &EncoderInfo {
		&self.info
	}

	#[tracing::instrument(skip_all, fields(name = "WebpEncoder::add_frame"))]
	fn add_frame(&mut self, frame: FrameRef) -> Result<(), EncoderError> {
		if self.first_duration.is_none() && self.encoder.is_none() {
			self.picture.width = frame.image.width() as _;
			self.picture.height = frame.image.height() as _;
			self.first_duration = Some(frame.duration_ts);
		} else if let Some(first_duration) = self.first_duration.take() {
			if self.static_image {
				return Err(EncoderError::MultipleFrames);
			}

			let encoder = SmartPtr::new(
				NonNull::new(unsafe {
					libwebp_sys::WebPAnimEncoderNew(self.picture.width, self.picture.height, &{
						let mut config = zero_memory_default::<libwebp_sys::WebPAnimEncoderOptions>();
						wrap_error(
							libwebp_sys::WebPAnimEncoderOptionsInit(&mut config),
							"failed to initialize webp anim encoder options",
						)?;

						config.anim_params.loop_count = match self.settings.loop_count {
							LoopCount::Finite(count) => count as _,
							LoopCount::Infinite => 0,
						};

						config.allow_mixed = match self.settings.quality {
							OutputQuality::High | OutputQuality::Auto => 1,
							_ => 0,
						};

						config
					})
				})
				.ok_or(WebPError::OutOfMemory)?,
				|encoder| {
					// Safety: The encoder is valid.
					unsafe {
						libwebp_sys::WebPAnimEncoderDelete(encoder.as_ptr());
					}
				},
			);

			self.encoder = Some(encoder);
			self.flush_frame(first_duration)?;
		}

		wrap_error(
			unsafe {
				libwebp_sys::WebPPictureImportRGBA(
					&mut *self.picture,
					frame.image.buf().as_ptr() as _,
					frame.image.width() as i32 * 4,
				)
			},
			"failed to import webp frame",
		)?;

		if self.encoder.is_some() {
			self.flush_frame(frame.duration_ts)?;
		}

		self.info.frame_count += 1;
		self.info.width = frame.image.width() as _;
		self.info.height = frame.image.height() as _;

		Ok(())
	}

	#[tracing::instrument(skip_all, fields(name = "WebpEncoder::finish"))]
	fn finish(mut self) -> Result<Vec<u8>, EncoderError> {
		let timestamp = self.timestamp();

		if self.encoder.is_none() && self.first_duration.is_none() {
			Err(EncoderError::NoFrames)
		} else if let Some(mut encoder) = self.encoder {
			wrap_error(
				unsafe {
					libwebp_sys::WebPAnimEncoderAdd(encoder.as_mut(), std::ptr::null_mut(), timestamp as _, &self.config)
				},
				"failed to add null webp frame",
			)?;

			let mut webp_data = SmartObject::new(zero_memory_default::<libwebp_sys::WebPData>(), |ptr| unsafe {
				libwebp_sys::WebPDataClear(ptr);
			});

			// Safety: The data is valid.
			unsafe { libwebp_sys::WebPDataInit(&mut *webp_data) };

			wrap_error(
				unsafe { libwebp_sys::WebPAnimEncoderAssemble(encoder.as_mut(), &mut *webp_data) },
				"failed to assemble webp",
			)?;

			let webp_data = webp_data.free();

			let mut data = NonNull::new(webp_data.bytes as _).ok_or(WebPError::OutOfMemory)?;

			// Safety: The data is valid and we are taking ownership of it.
			let vec = unsafe { std::vec::Vec::from_raw_parts(data.as_mut(), webp_data.size, webp_data.size) };

			Ok(vec)
		} else {
			let mut memory_writer = SmartObject::new(zero_memory_default::<libwebp_sys::WebPMemoryWriter>(), |ptr| unsafe {
				libwebp_sys::WebPMemoryWriterClear(ptr);
			});

			// Safety: The functions are correct, but the library requires picture.writer to
			// be a "safe" function and we only have a "unsafe" function.
			self.picture.writer = Some(unsafe {
				std::mem::transmute::<
					unsafe extern "C" fn(*const u8, usize, *const libwebp_sys::WebPPicture) -> i32,
					extern "C" fn(*const u8, usize, *const libwebp_sys::WebPPicture) -> i32,
				>(libwebp_sys::WebPMemoryWrite)
			});
			self.picture.custom_ptr = &mut *memory_writer as *mut _ as _;

			// Safety: The picture is valid.
			wrap_error(
				unsafe { libwebp_sys::WebPEncode(&self.config, &mut *self.picture) },
				"failed to encode webp",
			)?;

			let memory_writer = memory_writer.free();

			let mut data = NonNull::new(memory_writer.mem).ok_or(WebPError::OutOfMemory)?;

			// Safety: The data is valid and we are taking ownership of it.
			let vec = unsafe { std::vec::Vec::from_raw_parts(data.as_mut(), memory_writer.size, memory_writer.max_size) };

			Ok(vec)
		}
	}
}
