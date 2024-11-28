use std::ptr::NonNull;

use libavif_sys::{AVIF_QUALITY_BEST, AVIF_QUALITY_LOSSLESS, AVIF_SPEED_FASTEST, AVIF_SPEED_SLOWEST};
use scuffle_image_processor_proto::OutputQuality;

use super::{Encoder, EncoderError, EncoderInfo, EncoderSettings};
use crate::worker::process::frame::FrameRef;
use crate::worker::process::libavif::AvifError;
use crate::worker::process::smart_object::{SmartObject, SmartPtr};

pub struct AvifEncoder {
	encoder: SmartPtr<libavif_sys::avifEncoder>,
	image: SmartPtr<libavif_sys::avifImage>,
	rgb: Option<libavif_sys::avifRGBImage>,
	first_duration: Option<u64>,
	info: EncoderInfo,
	static_image: bool,
}

impl AvifEncoder {
	#[tracing::instrument(skip_all, fields(name = "AvifEncoder::new"))]
	pub fn new(settings: EncoderSettings) -> Result<Self, EncoderError> {
		let mut encoder = SmartPtr::new(
			NonNull::new(unsafe { libavif_sys::avifEncoderCreate() }).ok_or(AvifError::OutOfMemory)?,
			|ptr| unsafe { libavif_sys::avifEncoderDestroy(ptr.as_ptr()) },
		);

		encoder.as_mut().maxThreads = 1;
		encoder.as_mut().timescale = settings.timescale;
		encoder.as_mut().quality = match settings.quality {
			OutputQuality::Auto => 75_i32,
			OutputQuality::Lossless => AVIF_QUALITY_LOSSLESS as i32,
			OutputQuality::High => AVIF_QUALITY_BEST as i32,
			OutputQuality::Medium => 75,
			OutputQuality::Low => 50,
		};
		encoder.as_mut().qualityAlpha = AVIF_QUALITY_LOSSLESS as i32;
		encoder.as_mut().speed = match settings.quality {
			OutputQuality::Auto => 5_i32,
			OutputQuality::Lossless => AVIF_SPEED_SLOWEST as i32,
			OutputQuality::High => 4,
			OutputQuality::Medium => 7,
			OutputQuality::Low => AVIF_SPEED_FASTEST as i32,
		};

		let mut image = SmartPtr::new(
			NonNull::new(unsafe { libavif_sys::avifImageCreateEmpty() }).ok_or(AvifError::OutOfMemory)?,
			|ptr| unsafe { libavif_sys::avifImageDestroy(ptr.as_ptr()) },
		);

		image.as_mut().colorPrimaries = libavif_sys::AVIF_COLOR_PRIMARIES_BT709 as _;
		image.as_mut().transferCharacteristics = libavif_sys::AVIF_TRANSFER_CHARACTERISTICS_SRGB as _;
		image.as_mut().matrixCoefficients = libavif_sys::AVIF_MATRIX_COEFFICIENTS_BT601 as _;

		image.as_mut().yuvRange = libavif_sys::AVIF_RANGE_FULL;
		image.as_mut().yuvFormat = libavif_sys::AVIF_PIXEL_FORMAT_YUV444;
		image.as_mut().alphaPremultiplied = 0;
		image.as_mut().depth = 8;

		Ok(Self {
			encoder,
			image,
			rgb: None,
			first_duration: None,
			static_image: settings.static_image,
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

	fn flush_frame(&mut self, duration: u64, flags: u32) -> Result<(), EncoderError> {
		// Safety: The image is valid.
		AvifError::from_code(unsafe {
			libavif_sys::avifEncoderAddImage(self.encoder.as_mut(), self.image.as_mut(), duration, flags)
		})?;

		Ok(())
	}
}

impl Encoder for AvifEncoder {
	fn info(&self) -> &EncoderInfo {
		&self.info
	}

	#[tracing::instrument(skip_all, fields(name = "AvifEncoder::add_frame"))]
	fn add_frame(&mut self, frame: FrameRef) -> Result<(), EncoderError> {
		if self.rgb.is_none() {
			self.image.as_mut().width = frame.image.width() as u32;
			self.image.as_mut().height = frame.image.height() as u32;

			let mut rgb = libavif_sys::avifRGBImage::default();

			// Safety: The image is valid.
			unsafe {
				libavif_sys::avifRGBImageSetDefaults(&mut rgb, self.image.as_ref());
			}

			rgb.rowBytes = frame.image.stride() as u32 * 4;

			self.rgb = Some(rgb);
			self.first_duration = Some(frame.duration_ts);
		} else if let Some(first_duration) = self.first_duration.take() {
			if self.static_image {
				return Err(EncoderError::MultipleFrames);
			}

			// Flush the first frame to the encoder.
			// Safety: The image is valid.
			self.flush_frame(first_duration, libavif_sys::AVIF_ADD_IMAGE_FLAG_NONE)?;
		}

		let rgb = self.rgb.as_mut().unwrap();

		rgb.pixels = frame.image.buf().as_ptr() as _;

		// Safety: The image and rgb are valid.
		AvifError::from_code(unsafe { libavif_sys::avifImageRGBToYUV(self.image.as_mut(), rgb) })?;

		// On the first frame we dont want to flush the image to the encoder yet, this
		// is because we don't know if there will be more frames.
		if self.first_duration.is_none() {
			self.flush_frame(frame.duration_ts, libavif_sys::AVIF_ADD_IMAGE_FLAG_NONE)?;
		}

		self.info.frame_count += 1;
		self.info.duration += frame.duration_ts;
		self.info.width = frame.image.width();
		self.info.height = frame.image.height();

		Ok(())
	}

	#[tracing::instrument(skip_all, fields(name = "AvifEncoder::finish"))]
	fn finish(mut self) -> Result<Vec<u8>, EncoderError> {
		if self.rgb.is_none() {
			return Err(EncoderError::NoFrames);
		}

		if let Some(first_duration) = self.first_duration.take() {
			self.flush_frame(first_duration, libavif_sys::AVIF_ADD_IMAGE_FLAG_SINGLE)?;
		}

		let mut output = SmartObject::new(libavif_sys::avifRWData::default(), |ptr| unsafe {
			libavif_sys::avifRWDataFree(ptr)
		});

		AvifError::from_code(unsafe { libavif_sys::avifEncoderFinish(self.encoder.as_mut(), &mut *output) })?;

		let output = output.free();

		let mut data = NonNull::new(output.data).ok_or(AvifError::OutOfMemory)?;

		// Safety: The output is valid, and we own the data.
		let vec = unsafe { std::vec::Vec::from_raw_parts(data.as_mut(), output.size, output.size) };

		Ok(vec)
	}
}
