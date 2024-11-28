use ffmpeg_sys_next::*;

use crate::codec::EncoderCodec;
use crate::dict::Dictionary;
use crate::error::FfmpegError;
use crate::frame::Frame;
use crate::io::Output;
use crate::packet::Packet;
use crate::smart_object::SmartPtr;

pub struct Encoder {
	incoming_time_base: AVRational,
	outgoing_time_base: AVRational,
	encoder: SmartPtr<AVCodecContext>,
	stream_index: i32,
	average_duration: i64,
	previous_dts: i64,
}

/// Safety: `Encoder` can be sent between threads.
unsafe impl Send for Encoder {}

#[derive(Clone, Debug)]
pub struct VideoEncoderSettings {
	pub width: i32,
	pub height: i32,
	pub frame_rate: i32,
	pub pixel_format: AVPixelFormat,
	pub gop_size: Option<i32>,
	pub qmax: Option<i32>,
	pub qmin: Option<i32>,
	pub thread_count: Option<i32>,
	pub thread_type: Option<i32>,
	pub sample_aspect_ratio: Option<AVRational>,
	pub bitrate: Option<i64>,
	pub rc_min_rate: Option<i64>,
	pub rc_max_rate: Option<i64>,
	pub rc_buffer_size: Option<i32>,
	pub max_b_frames: Option<i32>,
	pub codec_specific_options: Option<Dictionary>,
	pub flags: Option<i32>,
	pub flags2: Option<i32>,
}

impl Default for VideoEncoderSettings {
	fn default() -> Self {
		Self {
			width: 0,
			height: 0,
			frame_rate: 0,
			pixel_format: AVPixelFormat::AV_PIX_FMT_NONE,
			gop_size: None,
			qmax: None,
			qmin: None,
			thread_count: None,
			thread_type: None,
			sample_aspect_ratio: None,
			bitrate: None,
			rc_min_rate: None,
			rc_max_rate: None,
			rc_buffer_size: None,
			max_b_frames: None,
			codec_specific_options: None,
			flags: None,
			flags2: None,
		}
	}
}

impl VideoEncoderSettings {
	pub fn builder(width: i32, height: i32, frame_rate: i32, pixel_format: AVPixelFormat) -> VideoEncoderBuilder {
		VideoEncoderBuilder::default()
			.dimentions(width, height)
			.frame_rate(frame_rate)
			.pixel_format(pixel_format)
	}
}

#[derive(Clone, Default, Debug)]
pub struct VideoEncoderBuilder(VideoEncoderSettings);

impl VideoEncoderBuilder {
	pub fn dimentions(mut self, width: i32, height: i32) -> Self {
		self.0.width = width;
		self.0.height = height;
		self
	}

	pub fn frame_rate(mut self, frame_rate: i32) -> Self {
		self.0.frame_rate = frame_rate;
		self
	}

	pub fn sample_aspect_ratio(mut self, sample_aspect_ratio: AVRational) -> Self {
		self.0.sample_aspect_ratio = Some(sample_aspect_ratio);
		self
	}

	pub fn gop_size(mut self, gop_size: i32) -> Self {
		self.0.gop_size = Some(gop_size);
		self
	}

	pub fn qmax(mut self, qmax: i32) -> Self {
		self.0.qmax = Some(qmax);
		self
	}

	pub fn qmin(mut self, qmin: i32) -> Self {
		self.0.qmin = Some(qmin);
		self
	}

	pub fn pixel_format(mut self, pixel_format: AVPixelFormat) -> Self {
		self.0.pixel_format = pixel_format;
		self
	}

	pub fn thread_count(mut self, thread_count: i32) -> Self {
		self.0.thread_count = Some(thread_count);
		self
	}

	pub fn thread_type(mut self, thread_type: i32) -> Self {
		self.0.thread_count = Some(thread_type);
		self
	}

	pub fn bitrate(mut self, bitrate: i64) -> Self {
		self.0.bitrate = Some(bitrate);
		self
	}

	pub fn rc_min_rate(mut self, rc_min_rate: i64) -> Self {
		self.0.rc_min_rate = Some(rc_min_rate);
		self
	}

	pub fn rc_max_rate(mut self, rc_max_rate: i64) -> Self {
		self.0.rc_max_rate = Some(rc_max_rate);
		self
	}

	pub fn rc_buffer_size(mut self, rc_buffer_size: i32) -> Self {
		self.0.rc_buffer_size = Some(rc_buffer_size);
		self
	}

	pub fn max_b_frames(mut self, max_b_frames: i32) -> Self {
		self.0.max_b_frames = Some(max_b_frames);
		self
	}

	pub fn codec_specific_options(mut self, codec_specific_options: Dictionary) -> Self {
		self.0.codec_specific_options = Some(codec_specific_options);
		self
	}

	pub fn flags(mut self, flags: i32) -> Self {
		self.0.flags = Some(flags);
		self
	}

	pub fn flags2(mut self, flags2: i32) -> Self {
		self.0.flags2 = Some(flags2);
		self
	}

	pub fn build(self) -> VideoEncoderSettings {
		self.0
	}
}

impl VideoEncoderSettings {
	fn apply(&self, encoder: &mut AVCodecContext) -> Result<(), FfmpegError> {
		if self.width <= 0 || self.height <= 0 || self.frame_rate <= 0 || self.pixel_format == AVPixelFormat::AV_PIX_FMT_NONE
		{
			return Err(FfmpegError::Arguments(
				"width, height, frame_rate and pixel_format must be set",
			));
		}

		encoder.width = self.width;
		encoder.height = self.height;

		encoder.pix_fmt = self.pixel_format;
		encoder.sample_aspect_ratio = self.sample_aspect_ratio.unwrap_or(encoder.sample_aspect_ratio);
		encoder.framerate = AVRational {
			num: self.frame_rate,
			den: 1,
		};
		encoder.thread_count = self.thread_count.unwrap_or(encoder.thread_count);
		encoder.thread_type = self.thread_type.unwrap_or(encoder.thread_type);
		encoder.gop_size = self.gop_size.unwrap_or(encoder.gop_size);
		encoder.qmax = self.qmax.unwrap_or(encoder.qmax);
		encoder.qmin = self.qmin.unwrap_or(encoder.qmin);
		encoder.bit_rate = self.bitrate.unwrap_or(encoder.bit_rate);
		encoder.rc_min_rate = self.rc_min_rate.unwrap_or(encoder.rc_min_rate);
		encoder.rc_max_rate = self.rc_max_rate.unwrap_or(encoder.rc_max_rate);
		encoder.rc_buffer_size = self.rc_buffer_size.unwrap_or(encoder.rc_buffer_size);
		encoder.max_b_frames = self.max_b_frames.unwrap_or(encoder.max_b_frames);
		encoder.flags = self.flags.unwrap_or(encoder.flags);
		encoder.flags2 = self.flags2.unwrap_or(encoder.flags2);

		Ok(())
	}

	fn average_duration(&self, timebase: AVRational) -> i64 {
		(timebase.den as i64) / (self.frame_rate as i64 * timebase.num as i64)
	}
}

#[derive(Clone, Debug)]
pub struct AudioEncoderSettings {
	pub sample_rate: i32,
	pub channel_layout: u64,
	pub channel_count: i32,
	pub sample_fmt: AVSampleFormat,
	pub thread_count: Option<i32>,
	pub thread_type: Option<i32>,
	pub bitrate: Option<i64>,
	pub buffer_size: Option<i64>,
	pub rc_min_rate: Option<i64>,
	pub rc_max_rate: Option<i64>,
	pub rc_buffer_size: Option<i32>,
	pub codec_specific_options: Option<Dictionary>,
	pub flags: Option<i32>,
	pub flags2: Option<i32>,
}

impl Default for AudioEncoderSettings {
	fn default() -> Self {
		Self {
			sample_rate: 0,
			channel_layout: 0,
			channel_count: 0,
			sample_fmt: AVSampleFormat::AV_SAMPLE_FMT_NONE,
			thread_count: None,
			thread_type: None,
			bitrate: None,
			buffer_size: None,
			rc_min_rate: None,
			rc_max_rate: None,
			rc_buffer_size: None,
			codec_specific_options: None,
			flags: None,
			flags2: None,
		}
	}
}

impl AudioEncoderSettings {
	pub fn builder(
		sample_rate: i32,
		channel_layout: u64,
		channel_count: i32,
		sample_fmt: AVSampleFormat,
	) -> AudioEncoderBuilder {
		AudioEncoderBuilder::default()
			.sample_rate(sample_rate)
			.channel_layout(channel_layout)
			.channel_count(channel_count)
			.sample_fmt(sample_fmt)
	}
}

#[derive(Clone, Default, Debug)]
pub struct AudioEncoderBuilder(AudioEncoderSettings);

impl AudioEncoderBuilder {
	pub fn sample_rate(mut self, sample_rate: i32) -> Self {
		self.0.sample_rate = sample_rate;
		self
	}

	pub fn channel_layout(mut self, channel_layout: u64) -> Self {
		self.0.channel_layout = channel_layout;
		self
	}

	pub fn channel_count(mut self, channel_count: i32) -> Self {
		self.0.channel_count = channel_count;
		self
	}

	pub fn sample_fmt(mut self, sample_fmt: AVSampleFormat) -> Self {
		self.0.sample_fmt = sample_fmt;
		self
	}

	pub fn thread_count(mut self, thread_count: i32) -> Self {
		self.0.thread_count = Some(thread_count);
		self
	}

	pub fn thread_type(mut self, thread_type: i32) -> Self {
		self.0.thread_count = Some(thread_type);
		self
	}

	pub fn bitrate(mut self, bitrate: i64) -> Self {
		self.0.bitrate = Some(bitrate);
		self
	}

	pub fn buffer_size(mut self, buffer_size: i64) -> Self {
		self.0.buffer_size = Some(buffer_size);
		self
	}

	pub fn rc_min_rate(mut self, rc_min_rate: i64) -> Self {
		self.0.rc_min_rate = Some(rc_min_rate);
		self
	}

	pub fn rc_max_rate(mut self, rc_max_rate: i64) -> Self {
		self.0.rc_max_rate = Some(rc_max_rate);
		self
	}

	pub fn rc_buffer_size(mut self, rc_buffer_size: i32) -> Self {
		self.0.rc_buffer_size = Some(rc_buffer_size);
		self
	}

	pub fn codec_specific_options(mut self, codec_specific_options: Dictionary) -> Self {
		self.0.codec_specific_options = Some(codec_specific_options);
		self
	}

	pub fn flags(mut self, flags: i32) -> Self {
		self.0.flags = Some(flags);
		self
	}

	pub fn flags2(mut self, flags2: i32) -> Self {
		self.0.flags2 = Some(flags2);
		self
	}

	pub fn build(self) -> AudioEncoderSettings {
		self.0
	}
}

impl AudioEncoderSettings {
	fn apply(&self, encoder: &mut AVCodecContext) -> Result<(), FfmpegError> {
		if self.sample_rate <= 0
			|| self.channel_layout == 0
			|| self.channel_count <= 0
			|| self.sample_fmt == AVSampleFormat::AV_SAMPLE_FMT_NONE
		{
			return Err(FfmpegError::Arguments(
				"sample_rate, channel_layout, channel_count and sample_fmt must be set",
			));
		}

		encoder.sample_rate = self.sample_rate;
		encoder.channel_layout = self.channel_layout;
		encoder.channels = self.channel_count;
		encoder.ch_layout.nb_channels = self.channel_count;
		encoder.sample_fmt = self.sample_fmt;
		encoder.thread_count = self.thread_count.unwrap_or(encoder.thread_count);
		encoder.thread_type = self.thread_type.unwrap_or(encoder.thread_type);
		encoder.bit_rate = self.bitrate.unwrap_or(encoder.bit_rate);
		encoder.rc_min_rate = self.rc_min_rate.unwrap_or(encoder.rc_min_rate);
		encoder.rc_max_rate = self.rc_max_rate.unwrap_or(encoder.rc_max_rate);
		encoder.rc_buffer_size = self.rc_buffer_size.unwrap_or(encoder.rc_buffer_size);
		encoder.flags = self.flags.unwrap_or(encoder.flags);
		encoder.flags2 = self.flags2.unwrap_or(encoder.flags2);

		Ok(())
	}

	fn average_duration(&self, timebase: AVRational) -> i64 {
		(timebase.den as i64) / (self.sample_rate as i64 * timebase.num as i64)
	}
}

#[derive(Clone, Debug)]
pub enum EncoderSettings {
	Video(VideoEncoderSettings),
	Audio(AudioEncoderSettings),
}

impl EncoderSettings {
	fn apply(&self, encoder: &mut AVCodecContext) -> Result<(), FfmpegError> {
		match self {
			EncoderSettings::Video(video_settings) => video_settings.apply(encoder),
			EncoderSettings::Audio(audio_settings) => audio_settings.apply(encoder),
		}
	}

	fn codec_specific_options(&mut self) -> Option<&mut Dictionary> {
		match self {
			EncoderSettings::Video(video_settings) => video_settings.codec_specific_options.as_mut(),
			EncoderSettings::Audio(audio_settings) => audio_settings.codec_specific_options.as_mut(),
		}
	}

	fn average_duration(&self, timebase: AVRational) -> i64 {
		match self {
			EncoderSettings::Video(video_settings) => video_settings.average_duration(timebase),
			EncoderSettings::Audio(audio_settings) => audio_settings.average_duration(timebase),
		}
	}
}

impl From<VideoEncoderSettings> for EncoderSettings {
	fn from(settings: VideoEncoderSettings) -> Self {
		EncoderSettings::Video(settings)
	}
}

impl From<AudioEncoderSettings> for EncoderSettings {
	fn from(settings: AudioEncoderSettings) -> Self {
		EncoderSettings::Audio(settings)
	}
}

impl Encoder {
	fn new<T: Send + Sync>(
		codec: EncoderCodec,
		output: &mut Output<T>,
		incoming_time_base: AVRational,
		outgoing_time_base: AVRational,
		settings: impl Into<EncoderSettings>,
	) -> Result<Self, FfmpegError> {
		if codec.as_ptr().is_null() {
			return Err(FfmpegError::NoEncoder);
		}

		let mut settings = settings.into();

		let global_header = output.flags() & AVFMT_GLOBALHEADER != 0;

		// Safety: `avcodec_alloc_context3` is safe to call, and the pointer returned is
		// valid.
		let mut encoder =
			unsafe { SmartPtr::wrap_non_null(avcodec_alloc_context3(codec.as_ptr()), |ptr| avcodec_free_context(ptr)) }
				.ok_or(FfmpegError::Alloc)?;

		let mut ost = output.add_stream(None).ok_or(FfmpegError::NoStream)?;

		let encoder_mut = encoder.as_deref_mut_except();

		encoder_mut.time_base = incoming_time_base;

		settings.apply(encoder_mut)?;

		if global_header {
			encoder_mut.flags |= AV_CODEC_FLAG_GLOBAL_HEADER as i32;
		}

		let codec_options = settings
			.codec_specific_options()
			.map(|options| options.as_mut_ptr_ref() as *mut *mut _)
			.unwrap_or(std::ptr::null_mut());

		// Safety: `avcodec_open2` is safe to call, 'encoder' and 'codec' and
		// 'codec_options' are a valid pointers.
		let res = unsafe { avcodec_open2(encoder_mut, codec.as_ptr(), codec_options) };
		if res < 0 {
			return Err(FfmpegError::Code(res.into()));
		}

		// Safety: `avcodec_parameters_from_context` is safe to call, 'ost' and
		// 'encoder' are valid pointers.
		let ret = unsafe { avcodec_parameters_from_context((*ost.as_mut_ptr()).codecpar, encoder_mut) };
		if ret < 0 {
			return Err(FfmpegError::Code(ret.into()));
		}

		ost.set_time_base(outgoing_time_base);

		let average_duration = settings.average_duration(outgoing_time_base);

		Ok(Self {
			incoming_time_base,
			outgoing_time_base,
			encoder,
			average_duration,
			stream_index: ost.index(),
			previous_dts: 0,
		})
	}

	pub fn send_eof(&mut self) -> Result<(), FfmpegError> {
		// Safety: `self.encoder` is a valid pointer.
		let ret = unsafe { avcodec_send_frame(self.encoder.as_mut_ptr(), std::ptr::null()) };
		if ret == 0 {
			Ok(())
		} else {
			Err(FfmpegError::Code(ret.into()))
		}
	}

	pub fn send_frame(&mut self, frame: &Frame) -> Result<(), FfmpegError> {
		// Safety: `self.encoder` and `frame` are valid pointers.
		let ret = unsafe { avcodec_send_frame(self.encoder.as_mut_ptr(), frame.as_ptr()) };
		if ret == 0 {
			Ok(())
		} else {
			Err(FfmpegError::Code(ret.into()))
		}
	}

	pub fn receive_packet(&mut self) -> Result<Option<Packet>, FfmpegError> {
		let mut packet = Packet::new()?;

		const AVERROR_EAGAIN: i32 = AVERROR(EAGAIN);

		// Safety: `self.encoder` and `packet` are valid pointers.
		let ret = unsafe { avcodec_receive_packet(self.encoder.as_mut_ptr(), packet.as_mut_ptr()) };

		match ret {
			AVERROR_EAGAIN | AVERROR_EOF => Ok(None),
			0 => {
				assert!(packet.dts().is_some(), "packet dts is none");
				let packet_dts = packet.dts().unwrap();
				assert!(
					packet_dts >= self.previous_dts,
					"packet dts is less than previous dts: {} >= {}",
					packet_dts,
					self.previous_dts
				);
				self.previous_dts = packet_dts;
				packet.rescale_timebase(self.incoming_time_base, self.outgoing_time_base);
				packet.set_stream_index(self.stream_index);
				Ok(Some(packet))
			}
			_ => Err(FfmpegError::Code(ret.into())),
		}
	}

	pub fn stream_index(&self) -> i32 {
		self.stream_index
	}

	pub fn incoming_time_base(&self) -> AVRational {
		self.incoming_time_base
	}

	pub fn outgoing_time_base(&self) -> AVRational {
		self.outgoing_time_base
	}
}

pub struct MuxerEncoder<T: Send + Sync> {
	encoder: Encoder,
	output: Output<T>,
	interleave: bool,
	muxer_headers_written: bool,
	muxer_options: Dictionary,
	buffered_packet: Option<Packet>,
	previous_dts: i64,
	previous_pts: i64,
}

#[derive(Clone, Debug)]
pub struct MuxerSettings {
	pub interleave: bool,
	pub muxer_options: Dictionary,
}

impl Default for MuxerSettings {
	fn default() -> Self {
		Self {
			interleave: true,
			muxer_options: Dictionary::new(),
		}
	}
}

impl MuxerSettings {
	pub fn builder() -> MuxerSettingsBuilder {
		MuxerSettingsBuilder::default()
	}
}

#[derive(Clone, Default, Debug)]
pub struct MuxerSettingsBuilder(MuxerSettings);

impl MuxerSettingsBuilder {
	pub fn interleave(mut self, interleave: bool) -> Self {
		self.0.interleave = interleave;
		self
	}

	pub fn muxer_options(mut self, muxer_options: Dictionary) -> Self {
		self.0.muxer_options = muxer_options;
		self
	}

	pub fn build(self) -> MuxerSettings {
		self.0
	}
}

impl<T: Send + Sync> MuxerEncoder<T> {
	pub fn new(
		codec: EncoderCodec,
		mut output: Output<T>,
		incoming_time_base: AVRational,
		outgoing_time_base: AVRational,
		settings: impl Into<EncoderSettings>,
		muxer_settings: MuxerSettings,
	) -> Result<Self, FfmpegError> {
		Ok(Self {
			encoder: Encoder::new(codec, &mut output, incoming_time_base, outgoing_time_base, settings)?,
			output,
			interleave: muxer_settings.interleave,
			muxer_options: muxer_settings.muxer_options,
			muxer_headers_written: false,
			previous_dts: -1,
			previous_pts: -1,
			buffered_packet: None,
		})
	}

	pub fn send_eof(&mut self) -> Result<(), FfmpegError> {
		self.encoder.send_eof()?;
		self.handle_packets()?;

		if let Some(mut bufferd_packet) = self.buffered_packet.take() {
			if let Some(dts) = bufferd_packet.dts() {
				if dts == self.previous_dts {
					bufferd_packet.set_dts(Some(dts + 1));
				}

				self.previous_dts = dts;
			}

			if let Some(pts) = bufferd_packet.pts() {
				if pts == self.previous_pts {
					bufferd_packet.set_pts(Some(pts + 1));
				}

				self.previous_pts = pts;
			}

			bufferd_packet.set_duration(Some(self.average_duration));

			if self.interleave {
				self.output.write_interleaved_packet(bufferd_packet)?;
			} else {
				self.output.write_packet(&bufferd_packet)?;
			}
		}

		if !self.muxer_headers_written {
			self.output.write_header_with_options(&mut self.muxer_options)?;
			self.muxer_headers_written = true;
		}

		self.output.write_trailer()?;
		Ok(())
	}

	pub fn send_frame(&mut self, frame: &Frame) -> Result<(), FfmpegError> {
		self.encoder.send_frame(frame)?;
		self.handle_packets()?;
		Ok(())
	}

	pub fn handle_packets(&mut self) -> Result<(), FfmpegError> {
		while let Some(packet) = self.encoder.receive_packet()? {
			if !self.muxer_headers_written {
				self.output.write_header_with_options(&mut self.muxer_options)?;
				self.muxer_headers_written = true;
			}

			if let Some(mut bufferd_packet) = self.buffered_packet.take() {
				if bufferd_packet.duration().unwrap_or(0) == 0 {
					match ((packet.dts(), bufferd_packet.dts()), (packet.pts(), bufferd_packet.pts())) {
						((Some(packet_dts), Some(bufferd_dts)), _) if bufferd_dts < packet_dts => {
							bufferd_packet.set_duration(Some(packet_dts - bufferd_dts))
						}
						(_, (Some(packet_pts), Some(bufferd_pts))) if bufferd_pts < packet_pts => {
							bufferd_packet.set_duration(Some(packet_pts - bufferd_pts))
						}
						_ => bufferd_packet.set_duration(Some(self.encoder.average_duration)),
					}
				}

				if let Some(dts) = bufferd_packet.dts() {
					if dts == self.previous_dts {
						bufferd_packet.set_dts(Some(dts + 1));
					}

					self.previous_dts = dts;
				}

				if let Some(pts) = bufferd_packet.pts() {
					if pts == self.previous_pts {
						bufferd_packet.set_pts(Some(pts + 1));
					}

					self.previous_pts = pts;
				}

				if self.interleave {
					self.output.write_interleaved_packet(bufferd_packet)?;
				} else {
					self.output.write_packet(&bufferd_packet)?;
				}
			}

			self.buffered_packet = Some(packet);
		}

		Ok(())
	}

	pub fn stream_index(&self) -> i32 {
		self.encoder.stream_index()
	}

	pub fn incoming_time_base(&self) -> AVRational {
		self.encoder.incoming_time_base()
	}

	pub fn outgoing_time_base(&self) -> AVRational {
		self.encoder.outgoing_time_base()
	}

	pub fn into_inner(self) -> Output<T> {
		self.output
	}
}

impl<T: Send + Sync> std::ops::Deref for MuxerEncoder<T> {
	type Target = Encoder;

	fn deref(&self) -> &Self::Target {
		&self.encoder
	}
}

impl<T: Send + Sync> std::ops::DerefMut for MuxerEncoder<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.encoder
	}
}
