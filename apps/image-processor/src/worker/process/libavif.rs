use rgb::ComponentBytes;

#[derive(Debug)]
pub struct AvifRgbImage(libavif_sys::avifRGBImage, Vec<rgb::RGBA8>);

impl AvifRgbImage {
	pub fn new(dec: &libavif_sys::avifDecoder) -> Result<Self, AvifError> {
		let mut img = libavif_sys::avifRGBImage::default();

		// Safety: The decoder is valid.
		unsafe {
			libavif_sys::avifRGBImageSetDefaults(&mut img, dec.image);
		};

		// Check the bit depth.
		if dec.image.is_null() || !dec.image.is_aligned() {
			return Err(AvifError::InvalidArgument);
		}

		let image = unsafe { &*dec.image };

		if image.depth != 8 {
			return Err(AvifError::UnsupportedBitDepth);
		}

		let channels = unsafe { libavif_sys::avifRGBFormatChannelCount(img.format) };

		assert_eq!(channels, 4, "unexpected channel count");

		let mut data = vec![rgb::RGBA::default(); img.width as usize * img.height as usize];

		img.pixels = data.as_bytes_mut().as_mut_ptr();
		img.rowBytes = img.width * 4;

		Ok(Self(img, data))
	}

	pub fn data(&self) -> &Vec<rgb::RGBA8> {
		&self.1
	}
}

impl std::ops::Deref for AvifRgbImage {
	type Target = libavif_sys::avifRGBImage;

	#[inline]
	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl std::ops::DerefMut for AvifRgbImage {
	#[inline]
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AvifError {
	#[error("unknown error {0}")]
	UnknownError(u32),
	#[error("invalid ftyp")]
	InvalidFtyp,
	#[error("no content")]
	NoContent,
	#[error("no yuv format selected")]
	NoYuvFormatSelected,
	#[error("reformat failed")]
	ReformatFailed,
	#[error("unsupported bit depth")]
	UnsupportedBitDepth,
	#[error("encode color failed")]
	EncodeColorFailed,
	#[error("encode alpha failed")]
	EncodeAlphaFailed,
	#[error("bmff parse failed")]
	BmffParseFailed,
	#[error("missing image item")]
	MissingImageItem,
	#[error("decode color failed")]
	DecodeColorFailed,
	#[error("decode alpha failed")]
	DecodeAlphaFailed,
	#[error("color alpha size mismatch")]
	ColorAlphaSizeMismatch,
	#[error("ispe size mismatch")]
	IspeSizeMismatch,
	#[error("no codec available")]
	NoCodecAvailable,
	#[error("no images remaining")]
	NoImagesRemaining,
	#[error("invalid exif payload")]
	InvalidExifPayload,
	#[error("invalid image grid")]
	InvalidImageGrid,
	#[error("invalid codec specific option")]
	InvalidCodecSpecificOption,
	#[error("truncated data")]
	TruncatedData,
	#[error("io not set")]
	IoNotSet,
	#[error("io error")]
	IoError,
	#[error("waiting on io")]
	WaitingOnIo,
	#[error("invalid argument")]
	InvalidArgument,
	#[error("not implemented")]
	NotImplemented,
	#[error("out of memory")]
	OutOfMemory,
	#[error("cannot change setting")]
	CannotChangeSetting,
	#[error("incompatible image")]
	IncompatibleImage,
}

impl AvifError {
	pub(crate) const fn from_code(code: u32) -> Result<(), AvifError> {
		match code {
			libavif_sys::AVIF_RESULT_OK => Ok(()),
			libavif_sys::AVIF_RESULT_INVALID_FTYP => Err(AvifError::InvalidFtyp),
			libavif_sys::AVIF_RESULT_NO_CONTENT => Err(AvifError::NoContent),
			libavif_sys::AVIF_RESULT_NO_YUV_FORMAT_SELECTED => Err(AvifError::NoYuvFormatSelected),
			libavif_sys::AVIF_RESULT_REFORMAT_FAILED => Err(AvifError::ReformatFailed),
			libavif_sys::AVIF_RESULT_UNSUPPORTED_DEPTH => Err(AvifError::UnsupportedBitDepth),
			libavif_sys::AVIF_RESULT_ENCODE_COLOR_FAILED => Err(AvifError::EncodeColorFailed),
			libavif_sys::AVIF_RESULT_ENCODE_ALPHA_FAILED => Err(AvifError::EncodeAlphaFailed),
			libavif_sys::AVIF_RESULT_BMFF_PARSE_FAILED => Err(AvifError::BmffParseFailed),
			libavif_sys::AVIF_RESULT_MISSING_IMAGE_ITEM => Err(AvifError::MissingImageItem),
			libavif_sys::AVIF_RESULT_DECODE_COLOR_FAILED => Err(AvifError::DecodeColorFailed),
			libavif_sys::AVIF_RESULT_DECODE_ALPHA_FAILED => Err(AvifError::DecodeAlphaFailed),
			libavif_sys::AVIF_RESULT_COLOR_ALPHA_SIZE_MISMATCH => Err(AvifError::ColorAlphaSizeMismatch),
			libavif_sys::AVIF_RESULT_ISPE_SIZE_MISMATCH => Err(AvifError::IspeSizeMismatch),
			libavif_sys::AVIF_RESULT_NO_CODEC_AVAILABLE => Err(AvifError::NoCodecAvailable),
			libavif_sys::AVIF_RESULT_NO_IMAGES_REMAINING => Err(AvifError::NoImagesRemaining),
			libavif_sys::AVIF_RESULT_INVALID_EXIF_PAYLOAD => Err(AvifError::InvalidExifPayload),
			libavif_sys::AVIF_RESULT_INVALID_IMAGE_GRID => Err(AvifError::InvalidImageGrid),
			libavif_sys::AVIF_RESULT_INVALID_CODEC_SPECIFIC_OPTION => Err(AvifError::InvalidCodecSpecificOption),
			libavif_sys::AVIF_RESULT_TRUNCATED_DATA => Err(AvifError::TruncatedData),
			libavif_sys::AVIF_RESULT_IO_NOT_SET => Err(AvifError::IoNotSet),
			libavif_sys::AVIF_RESULT_IO_ERROR => Err(AvifError::IoError),
			libavif_sys::AVIF_RESULT_WAITING_ON_IO => Err(AvifError::WaitingOnIo),
			libavif_sys::AVIF_RESULT_INVALID_ARGUMENT => Err(AvifError::InvalidArgument),
			libavif_sys::AVIF_RESULT_NOT_IMPLEMENTED => Err(AvifError::NotImplemented),
			libavif_sys::AVIF_RESULT_OUT_OF_MEMORY => Err(AvifError::OutOfMemory),
			libavif_sys::AVIF_RESULT_CANNOT_CHANGE_SETTING => Err(AvifError::CannotChangeSetting),
			libavif_sys::AVIF_RESULT_INCOMPATIBLE_IMAGE => Err(AvifError::IncompatibleImage),
			e => Err(AvifError::UnknownError(e)),
		}
	}
}
