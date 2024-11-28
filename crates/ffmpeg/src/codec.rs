use ffmpeg_sys_next::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct DecoderCodec(*const AVCodec);

impl std::fmt::Debug for DecoderCodec {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		if self.0.is_null() {
			return f
				.debug_struct("DecoderCodec")
				.field("name", &c"null")
				.field("id", &AVCodecID::AV_CODEC_ID_NONE)
				.finish();
		}

		// Safety: `self.0` is a non-null pointer.
		let name = unsafe { std::ffi::CStr::from_ptr((*self.0).name) };
		f.debug_struct("DecoderCodec")
			.field("name", &name)
			// Safety: `self.0` is a non-null pointer.
			.field("id", unsafe { &(*self.0).id })
			.finish()
	}
}

impl DecoderCodec {
	pub fn empty() -> Self {
		Self(std::ptr::null())
	}

	pub fn new(codec_id: AVCodecID) -> Option<Self> {
		// Safety: `avcodec_find_decoder` is safe to call.
		let codec = unsafe { avcodec_find_decoder(codec_id) };
		if codec.is_null() {
			None
		} else {
			Some(Self(codec))
		}
	}

	pub fn by_name(name: &str) -> Option<Self> {
		let c_name = std::ffi::CString::new(name).ok()?;

		// Safety: `avcodec_find_decoder_by_name` is safe to call with a valid c-string.
		let codec = unsafe { avcodec_find_decoder_by_name(c_name.as_ptr()) };
		if codec.is_null() {
			None
		} else {
			Some(Self(codec))
		}
	}

	pub fn as_ptr(&self) -> *const AVCodec {
		self.0
	}

	pub fn from_ptr(ptr: *const AVCodec) -> Self {
		Self(ptr)
	}
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct EncoderCodec(*const AVCodec);

impl std::fmt::Debug for EncoderCodec {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		if self.0.is_null() {
			return f
				.debug_struct("EncoderCodec")
				.field("name", &c"null")
				.field("id", &AVCodecID::AV_CODEC_ID_NONE)
				.finish();
		}

		// Safety: `self.0` is a valid pointer.
		let name = unsafe { std::ffi::CStr::from_ptr((*self.0).name) };
		f.debug_struct("EncoderCodec")
			.field("name", &name)
			// Safety: `self.0` is a valid pointer.
			.field("id", unsafe { &(*self.0).id })
			.finish()
	}
}

impl EncoderCodec {
	pub fn empty() -> Self {
		Self(std::ptr::null())
	}

	pub fn new(codec_id: AVCodecID) -> Option<Self> {
		// Safety: `avcodec_find_encoder` is safe to call.
		let codec = unsafe { avcodec_find_encoder(codec_id) };
		if codec.is_null() {
			None
		} else {
			Some(Self(codec))
		}
	}

	pub fn by_name(name: &str) -> Option<Self> {
		let c_name = std::ffi::CString::new(name).ok()?;
		// Safety: `avcodec_find_encoder_by_name` is safe to call with a valid c-string.
		let codec = unsafe { avcodec_find_encoder_by_name(c_name.as_ptr()) };
		if codec.is_null() {
			None
		} else {
			Some(Self(codec))
		}
	}

	pub fn as_ptr(&self) -> *const AVCodec {
		self.0
	}

	pub fn from_ptr(ptr: *const AVCodec) -> Self {
		Self(ptr)
	}
}

impl From<EncoderCodec> for *const AVCodec {
	fn from(codec: EncoderCodec) -> Self {
		codec.0
	}
}

impl From<DecoderCodec> for *const AVCodec {
	fn from(codec: DecoderCodec) -> Self {
		codec.0
	}
}
