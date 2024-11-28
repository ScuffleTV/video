use ffmpeg_sys_next::*;

use crate::codec::DecoderCodec;
use crate::error::{FfmpegError, AVERROR_EAGAIN};
use crate::frame::{Frame, VideoFrame};
use crate::packet::Packet;
use crate::smart_object::SmartPtr;
use crate::stream::Stream;

#[derive(Debug)]
pub enum Decoder {
	Video(VideoDecoder),
	Audio(AudioDecoder),
}

pub struct GenericDecoder {
	decoder: SmartPtr<AVCodecContext>,
}

/// Safety: `GenericDecoder` can be sent between threads.
unsafe impl Send for GenericDecoder {}

impl std::fmt::Debug for GenericDecoder {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Decoder")
			.field("time_base", &self.time_base())
			.field("codec_type", &self.codec_type())
			.finish()
	}
}

pub struct VideoDecoder(GenericDecoder);

impl std::fmt::Debug for VideoDecoder {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("VideoDecoder")
			.field("time_base", &self.time_base())
			.field("width", &self.width())
			.field("height", &self.height())
			.field("pixel_format", &self.pixel_format())
			.field("frame_rate", &self.frame_rate())
			.field("sample_aspect_ratio", &self.sample_aspect_ratio())
			.finish()
	}
}

pub struct AudioDecoder(GenericDecoder);

impl std::fmt::Debug for AudioDecoder {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("AudioDecoder")
			.field("time_base", &self.time_base())
			.field("sample_rate", &self.sample_rate())
			.field("channel_layout", &self.channel_layout())
			.field("channels", &self.channels())
			.field("sample_fmt", &self.sample_format())
			.finish()
	}
}

pub struct DecoderOptions {
	pub codec: Option<DecoderCodec>,
	pub thread_count: i32,
}

impl Default for DecoderOptions {
	fn default() -> Self {
		Self {
			codec: None,
			thread_count: 1,
		}
	}
}

impl Decoder {
	pub fn new(ist: &Stream) -> Result<Self, FfmpegError> {
		Self::with_options(ist, Default::default())
	}

	pub fn with_options(ist: &Stream, options: DecoderOptions) -> Result<Self, FfmpegError> {
		let Some(codec_params) = ist.codec_parameters() else {
			return Err(FfmpegError::NoDecoder);
		};

		let codec = options
			.codec
			.or_else(|| DecoderCodec::new(codec_params.codec_id))
			.ok_or(FfmpegError::NoDecoder)?;
		if codec.as_ptr().is_null() {
			return Err(FfmpegError::NoDecoder);
		}

		// Safety: `codec` is a valid pointer, also the pointer returned from
		// `avcodec_alloc_context3` is valid.
		let mut decoder =
			unsafe { SmartPtr::wrap_non_null(avcodec_alloc_context3(codec.as_ptr()), |ptr| avcodec_free_context(ptr)) }
				.ok_or(FfmpegError::Alloc)?;

		// Safety: `codec_params` is a valid pointer, and `decoder` is a valid pointer.
		let ret = unsafe { avcodec_parameters_to_context(decoder.as_mut_ptr(), codec_params) };
		if ret < 0 {
			return Err(FfmpegError::Code(ret.into()));
		}

		let decoder_mut = decoder.as_deref_mut_except();

		decoder_mut.pkt_timebase = ist.time_base();
		decoder_mut.time_base = ist.time_base();
		decoder_mut.thread_count = options.thread_count;

		if decoder_mut.codec_type == AVMediaType::AVMEDIA_TYPE_VIDEO {
			// Even though we are upcasting `AVFormatContext` from a const pointer to a
			// mutable pointer, it is still safe becasuse av_guess_frame_rate does not use
			// the pointer to modify the `AVFormatContext`. https://github.com/FFmpeg/FFmpeg/blame/90bef6390fba02472141f299264331f68018a992/libavformat/avformat.c#L728
			// The function does not use the pointer at all, it only uses the `AVStream`
			// pointer to get the `AVRational`
			decoder_mut.framerate = unsafe {
				av_guess_frame_rate(
					ist.format_context() as *const AVFormatContext as *mut AVFormatContext,
					ist.as_ptr() as *mut AVStream,
					std::ptr::null_mut(),
				)
			};
		}

		if matches!(
			decoder_mut.codec_type,
			AVMediaType::AVMEDIA_TYPE_VIDEO | AVMediaType::AVMEDIA_TYPE_AUDIO
		) {
			// Safety: `codec` is a valid pointer, and `decoder` is a valid pointer.
			let ret = unsafe { avcodec_open2(decoder_mut, codec.as_ptr(), std::ptr::null_mut()) };
			if ret < 0 {
				return Err(FfmpegError::Code(ret.into()));
			}
		}

		Ok(match decoder_mut.codec_type {
			AVMediaType::AVMEDIA_TYPE_VIDEO => Self::Video(VideoDecoder(GenericDecoder { decoder })),
			AVMediaType::AVMEDIA_TYPE_AUDIO => Self::Audio(AudioDecoder(GenericDecoder { decoder })),
			_ => Err(FfmpegError::NoDecoder)?,
		})
	}
}

impl GenericDecoder {
	pub fn codec_type(&self) -> AVMediaType {
		self.decoder.as_deref_except().codec_type
	}

	pub fn time_base(&self) -> AVRational {
		self.decoder.as_deref_except().time_base
	}

	pub fn send_packet(&mut self, packet: &Packet) -> Result<(), FfmpegError> {
		// Safety: `packet` is a valid pointer, and `self.decoder` is a valid pointer.
		let ret = unsafe { avcodec_send_packet(self.decoder.as_mut_ptr(), packet.as_ptr()) };

		match ret {
			0 => Ok(()),
			_ => Err(FfmpegError::Code(ret.into())),
		}
	}

	pub fn send_eof(&mut self) -> Result<(), FfmpegError> {
		// Safety: `self.decoder` is a valid pointer.
		let ret = unsafe { avcodec_send_packet(self.decoder.as_mut_ptr(), std::ptr::null()) };

		match ret {
			0 => Ok(()),
			_ => Err(FfmpegError::Code(ret.into())),
		}
	}

	pub fn receive_frame(&mut self) -> Result<Option<VideoFrame>, FfmpegError> {
		let mut frame = Frame::new()?;

		// Safety: `frame` is a valid pointer, and `self.decoder` is a valid pointer.
		let ret = unsafe { avcodec_receive_frame(self.decoder.as_mut_ptr(), frame.as_mut_ptr()) };

		match ret {
			AVERROR_EAGAIN | AVERROR_EOF => Ok(None),
			0 => {
				frame.set_time_base(self.decoder.as_deref_except().time_base);
				Ok(Some(frame.video()))
			}
			_ => Err(FfmpegError::Code(ret.into())),
		}
	}
}

impl VideoDecoder {
	pub fn width(&self) -> i32 {
		self.0.decoder.as_deref_except().width
	}

	pub fn height(&self) -> i32 {
		self.0.decoder.as_deref_except().height
	}

	pub fn pixel_format(&self) -> AVPixelFormat {
		self.0.decoder.as_deref_except().pix_fmt
	}

	pub fn frame_rate(&self) -> AVRational {
		self.0.decoder.as_deref_except().framerate
	}

	pub fn sample_aspect_ratio(&self) -> AVRational {
		self.0.decoder.as_deref_except().sample_aspect_ratio
	}
}

impl std::ops::Deref for VideoDecoder {
	type Target = GenericDecoder;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl std::ops::DerefMut for VideoDecoder {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl AudioDecoder {
	pub fn sample_rate(&self) -> i32 {
		self.0.decoder.as_deref_except().sample_rate
	}

	pub fn channel_layout(&self) -> u64 {
		self.0.decoder.as_deref_except().channel_layout
	}

	pub fn channels(&self) -> i32 {
		self.0.decoder.as_deref_except().channels
	}

	pub fn sample_format(&self) -> AVSampleFormat {
		self.0.decoder.as_deref_except().sample_fmt
	}
}

impl std::ops::Deref for AudioDecoder {
	type Target = GenericDecoder;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl std::ops::DerefMut for AudioDecoder {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}
