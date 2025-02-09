use std::ptr::NonNull;

use ffmpeg_sys_next::*;

use crate::codec::EncoderCodec;
use crate::dict::Dictionary;
use crate::error::{FfmpegError, FfmpegErrorCode};
use crate::frame::{AudioChannelLayout, Frame};
use crate::io::Output;
use crate::packet::Packet;
use crate::smart_object::SmartPtr;

/// Represents an encoder.
pub struct Encoder {
    incoming_time_base: AVRational,
    outgoing_time_base: AVRational,
    encoder: SmartPtr<AVCodecContext>,
    stream_index: i32,
    previous_dts: i64,
}

/// Safety: `Encoder` can be sent between threads.
unsafe impl Send for Encoder {}

/// Represents the settings for a video encoder.
#[derive(bon::Builder)]
pub struct VideoEncoderSettings {
    width: i32,
    height: i32,
    frame_rate: AVRational,
    pixel_format: AVPixelFormat,
    gop_size: Option<i32>,
    qmax: Option<i32>,
    qmin: Option<i32>,
    thread_count: Option<i32>,
    thread_type: Option<i32>,
    sample_aspect_ratio: Option<AVRational>,
    bitrate: Option<i64>,
    rc_min_rate: Option<i64>,
    rc_max_rate: Option<i64>,
    rc_buffer_size: Option<i32>,
    max_b_frames: Option<i32>,
    codec_specific_options: Option<Dictionary>,
    flags: Option<i32>,
    flags2: Option<i32>,
}

impl VideoEncoderSettings {
    fn apply(self, encoder: &mut AVCodecContext) -> Result<(), FfmpegError> {
        if self.width <= 0
            || self.height <= 0
            || self.frame_rate.num <= 0
            || self.frame_rate.den <= 0
            || self.pixel_format == AVPixelFormat::AV_PIX_FMT_NONE
        {
            return Err(FfmpegError::Arguments(
                "width, height, frame_rate and pixel_format must be set",
            ));
        }

        encoder.width = self.width;
        encoder.height = self.height;
        encoder.pix_fmt = self.pixel_format;
        encoder.sample_aspect_ratio = self.sample_aspect_ratio.unwrap_or(encoder.sample_aspect_ratio);
        encoder.framerate = self.frame_rate;
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
}

/// Represents the settings for an audio encoder.
#[derive(bon::Builder)]
pub struct AudioEncoderSettings {
    sample_rate: i32,
    ch_layout: AudioChannelLayout,
    sample_fmt: AVSampleFormat,
    thread_count: Option<i32>,
    thread_type: Option<i32>,
    bitrate: Option<i64>,
    rc_min_rate: Option<i64>,
    rc_max_rate: Option<i64>,
    rc_buffer_size: Option<i32>,
    codec_specific_options: Option<Dictionary>,
    flags: Option<i32>,
    flags2: Option<i32>,
}

impl AudioEncoderSettings {
    fn apply(self, encoder: &mut AVCodecContext) -> Result<(), FfmpegError> {
        if self.sample_rate <= 0 || self.sample_fmt == AVSampleFormat::AV_SAMPLE_FMT_NONE {
            return Err(FfmpegError::Arguments(
                "sample_rate, channel_layout and sample_fmt must be set",
            ));
        }

        encoder.sample_rate = self.sample_rate;
        self.ch_layout.apply(&mut encoder.ch_layout);
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
}

/// Represents the settings for an encoder.
pub enum EncoderSettings {
    /// Video encoder settings.
    Video(VideoEncoderSettings),
    /// Audio encoder settings.
    Audio(AudioEncoderSettings),
}

impl EncoderSettings {
    fn apply(self, encoder: &mut AVCodecContext) -> Result<(), FfmpegError> {
        match self {
            EncoderSettings::Video(video_settings) => video_settings.apply(encoder),
            EncoderSettings::Audio(audio_settings) => audio_settings.apply(encoder),
        }
    }

    const fn codec_specific_options(&mut self) -> Option<&mut Dictionary> {
        match self {
            EncoderSettings::Video(video_settings) => video_settings.codec_specific_options.as_mut(),
            EncoderSettings::Audio(audio_settings) => audio_settings.codec_specific_options.as_mut(),
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
    /// Creates a new encoder.
    pub fn new<T: Send + Sync>(
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

        let destructor = |ptr: &mut *mut AVCodecContext| {
            // Safety: `avcodec_free_context` is safe to call when the pointer is valid, and it is because it comes from `avcodec_alloc_context3`.
            unsafe { avcodec_free_context(ptr) };
        };

        // Safety: `avcodec_alloc_context3` is safe to call.
        let encoder = unsafe { avcodec_alloc_context3(codec.as_ptr()) };

        // Safety: The pointer here is valid and the destructor has been setup to handle the cleanup.
        let mut encoder = unsafe { SmartPtr::wrap_non_null(encoder, destructor) }.ok_or(FfmpegError::Alloc)?;

        let mut ost = output.add_stream(None).ok_or(FfmpegError::NoStream)?;

        let encoder_mut = encoder.as_deref_mut_except();

        encoder_mut.time_base = incoming_time_base;

        let mut codec_options = settings.codec_specific_options().cloned();

        let codec_options_ptr = codec_options
            .as_mut()
            .map(|options| options.as_mut_ptr_ref() as *mut *mut _)
            .unwrap_or(std::ptr::null_mut());

        settings.apply(encoder_mut)?;

        if global_header {
            encoder_mut.flags |= AV_CODEC_FLAG_GLOBAL_HEADER as i32;
        }

        // Safety: `avcodec_open2` is safe to call, 'encoder' and 'codec' and
        // 'codec_options_ptr' are a valid pointers.
        FfmpegErrorCode(unsafe { avcodec_open2(encoder_mut, codec.as_ptr(), codec_options_ptr) }).result()?;

        // Safety: The pointer here is valid.
        let ost_mut = unsafe { NonNull::new(ost.as_mut_ptr()).ok_or(FfmpegError::NoStream)?.as_mut() };

        // Safety: `avcodec_parameters_from_context` is safe to call, 'ost' and
        // 'encoder' are valid pointers.
        FfmpegErrorCode(unsafe { avcodec_parameters_from_context(ost_mut.codecpar, encoder_mut) }).result()?;

        ost.set_time_base(outgoing_time_base);

        Ok(Self {
            incoming_time_base,
            outgoing_time_base,
            encoder,
            stream_index: ost.index(),
            previous_dts: 0,
        })
    }

    /// Sends an EOF frame to the encoder.
    pub fn send_eof(&mut self) -> Result<(), FfmpegError> {
        // Safety: `self.encoder` is a valid pointer.
        FfmpegErrorCode(unsafe { avcodec_send_frame(self.encoder.as_mut_ptr(), std::ptr::null()) }).result()?;
        Ok(())
    }

    /// Sends a frame to the encoder.
    pub fn send_frame(&mut self, frame: &Frame) -> Result<(), FfmpegError> {
        // Safety: `self.encoder` and `frame` are valid pointers.
        FfmpegErrorCode(unsafe { avcodec_send_frame(self.encoder.as_mut_ptr(), frame.as_ptr()) }).result()?;
        Ok(())
    }

    /// Receives a packet from the encoder.
    pub fn receive_packet(&mut self) -> Result<Option<Packet>, FfmpegError> {
        let mut packet = Packet::new()?;

        // Safety: `self.encoder` and `packet` are valid pointers.
        let ret = FfmpegErrorCode(unsafe { avcodec_receive_packet(self.encoder.as_mut_ptr(), packet.as_mut_ptr()) });

        match ret {
            FfmpegErrorCode::Eagain | FfmpegErrorCode::Eof => Ok(None),
            code if code.is_success() => {
                if cfg!(debug_assertions) {
                    debug_assert!(
                        packet.dts().is_some(),
                        "packet dts is none, this should never happen, please report this bug"
                    );
                    let packet_dts = packet.dts().unwrap();
                    debug_assert!(
                        packet_dts >= self.previous_dts,
                        "packet dts is less than previous dts: {} >= {}",
                        packet_dts,
                        self.previous_dts
                    );
                    self.previous_dts = packet_dts;
                }

                packet.convert_timebase(self.incoming_time_base, self.outgoing_time_base);
                packet.set_stream_index(self.stream_index);
                Ok(Some(packet))
            }
            code => Err(FfmpegError::Code(code)),
        }
    }

    /// Returns the stream index of the encoder.
    pub const fn stream_index(&self) -> i32 {
        self.stream_index
    }

    /// Returns the incoming time base of the encoder.
    pub const fn incoming_time_base(&self) -> AVRational {
        self.incoming_time_base
    }

    /// Returns the outgoing time base of the encoder.
    pub const fn outgoing_time_base(&self) -> AVRational {
        self.outgoing_time_base
    }
}

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use std::io::Write;

    use bytes::{Buf, Bytes};
    use ffmpeg_sys_next::AVCodecID::AV_CODEC_ID_MPEG4;
    use ffmpeg_sys_next::{AVCodecContext, AVMediaType, AVPixelFormat, AVRational, AVSampleFormat};
    use sha2::Digest;

    use crate::codec::EncoderCodec;
    use crate::decoder::Decoder;
    use crate::dict::Dictionary;
    use crate::encoder::{AudioChannelLayout, AudioEncoderSettings, Encoder, EncoderSettings, VideoEncoderSettings};
    use crate::error::FfmpegError;
    use crate::io::{Input, Output, OutputOptions};

    #[test]
    fn test_video_encoder_apply() {
        let width = 1920;
        let height = 1080;
        let frame_rate = 30;
        let pixel_format = AVPixelFormat::AV_PIX_FMT_YUV420P;
        let sample_aspect_ratio = AVRational { num: 1, den: 1 };
        let gop_size = 12;
        let qmax = 31;
        let qmin = 1;
        let thread_count = 4;
        let thread_type = 2;
        let bitrate = 8_000;
        let rc_min_rate = 500_000;
        let rc_max_rate = 2_000_000;
        let rc_buffer_size = 1024;
        let max_b_frames = 3;
        let mut codec_specific_options = Dictionary::new();
        let _ = codec_specific_options.set("preset", "ultrafast");
        let _ = codec_specific_options.set("crf", "23");
        let flags = 0x01;
        let flags2 = 0x02;

        let settings = VideoEncoderSettings::builder()
            .width(width)
            .height(height)
            .frame_rate(AVRational { num: frame_rate, den: 1 })
            .pixel_format(pixel_format)
            .sample_aspect_ratio(sample_aspect_ratio)
            .gop_size(gop_size)
            .qmax(qmax)
            .qmin(qmin)
            .thread_count(thread_count)
            .thread_type(thread_type)
            .bitrate(bitrate)
            .rc_min_rate(rc_min_rate)
            .rc_max_rate(rc_max_rate)
            .rc_buffer_size(rc_buffer_size)
            .max_b_frames(max_b_frames)
            .codec_specific_options(codec_specific_options)
            .flags(flags)
            .flags2(flags2)
            .build();

        assert_eq!(settings.width, width);
        assert_eq!(settings.height, height);
        assert_eq!(settings.frame_rate, AVRational { num: frame_rate, den: 1 });
        assert_eq!(settings.pixel_format, pixel_format);
        assert_eq!(settings.sample_aspect_ratio, Some(sample_aspect_ratio));
        assert_eq!(settings.gop_size, Some(gop_size));
        assert_eq!(settings.qmax, Some(qmax));
        assert_eq!(settings.qmin, Some(qmin));
        assert_eq!(settings.thread_count, Some(thread_count));
        assert_eq!(settings.thread_type, Some(thread_type));
        assert_eq!(settings.bitrate, Some(bitrate));
        assert_eq!(settings.rc_min_rate, Some(rc_min_rate));
        assert_eq!(settings.rc_max_rate, Some(rc_max_rate));
        assert_eq!(settings.rc_buffer_size, Some(rc_buffer_size));
        assert_eq!(settings.max_b_frames, Some(max_b_frames));
        assert!(settings.codec_specific_options.is_some());
        let actual_codec_specific_options = settings.codec_specific_options.as_ref().unwrap();
        assert_eq!(actual_codec_specific_options.get(c"preset"), Some(c"ultrafast"));
        assert_eq!(actual_codec_specific_options.get(c"crf"), Some(c"23"));
        assert_eq!(settings.flags, Some(flags));
        assert_eq!(settings.flags2, Some(flags2));

        // Safety: We are zeroing the memory for the encoder context.
        let mut encoder = unsafe { std::mem::zeroed::<AVCodecContext>() };
        let result = settings.apply(&mut encoder);
        assert!(result.is_ok(), "Failed to apply settings: {:?}", result.err());

        assert_eq!(encoder.width, width);
        assert_eq!(encoder.height, height);
        assert_eq!(encoder.pix_fmt, pixel_format);
        assert_eq!(encoder.sample_aspect_ratio, sample_aspect_ratio);
        assert_eq!(encoder.framerate.num, frame_rate);
        assert_eq!(encoder.framerate.den, 1);
        assert_eq!(encoder.thread_count, thread_count);
        assert_eq!(encoder.thread_type, thread_type);
        assert_eq!(encoder.gop_size, gop_size);
        assert_eq!(encoder.qmax, qmax);
        assert_eq!(encoder.qmin, qmin);
        assert_eq!(encoder.bit_rate, bitrate);
        assert_eq!(encoder.rc_min_rate, rc_min_rate);
        assert_eq!(encoder.rc_max_rate, rc_max_rate);
        assert_eq!(encoder.rc_buffer_size, rc_buffer_size);
        assert_eq!(encoder.max_b_frames, max_b_frames);
        assert_eq!(encoder.flags, flags);
        assert_eq!(encoder.flags2, flags2);
    }

    #[test]
    fn test_video_encoder_settings_apply_error() {
        let settings = VideoEncoderSettings::builder()
            .width(0)
            .height(0)
            .pixel_format(AVPixelFormat::AV_PIX_FMT_YUV420P)
            .frame_rate(AVRational { num: 0, den: 1 })
            .build();
        // Safety: We are zeroing the memory for the encoder context.
        let mut encoder = unsafe { std::mem::zeroed::<AVCodecContext>() };
        let result = settings.apply(&mut encoder);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            FfmpegError::Arguments("width, height, frame_rate and pixel_format must be set")
        );
    }

    #[test]
    fn test_audio_encoder_apply() {
        let sample_rate = 44100;
        let channel_count = 2;
        let sample_fmt = AVSampleFormat::AV_SAMPLE_FMT_S16;
        let thread_count = 4;
        let thread_type = 1;
        let bitrate = 128_000;
        let rc_min_rate = 64_000;
        let rc_max_rate = 256_000;
        let rc_buffer_size = 1024;
        let flags = 0x01;
        let flags2 = 0x02;

        let mut codec_specific_options = Dictionary::new();
        codec_specific_options
            .set(c"profile", c"high")
            .expect("Failed to set profile");

        let settings = AudioEncoderSettings::builder()
            .sample_rate(sample_rate)
            .ch_layout(AudioChannelLayout::new(channel_count).expect("channel_count is a valid value"))
            .sample_fmt(sample_fmt)
            .thread_count(thread_count)
            .thread_type(thread_type)
            .bitrate(bitrate)
            .rc_min_rate(rc_min_rate)
            .rc_max_rate(rc_max_rate)
            .rc_buffer_size(rc_buffer_size)
            .codec_specific_options(codec_specific_options)
            .flags(flags)
            .flags2(flags2)
            .build();

        assert_eq!(settings.sample_rate, sample_rate);
        assert_eq!(settings.ch_layout.channel_count(), 2);
        assert_eq!(settings.sample_fmt, sample_fmt);
        assert_eq!(settings.thread_count, Some(thread_count));
        assert_eq!(settings.thread_type, Some(thread_type));
        assert_eq!(settings.bitrate, Some(bitrate));
        assert_eq!(settings.rc_min_rate, Some(rc_min_rate));
        assert_eq!(settings.rc_max_rate, Some(rc_max_rate));
        assert_eq!(settings.rc_buffer_size, Some(rc_buffer_size));
        assert!(settings.codec_specific_options.is_some());

        let actual_codec_specific_options = settings.codec_specific_options.unwrap();
        assert_eq!(actual_codec_specific_options.get(c"profile"), Some(c"high"));

        assert_eq!(settings.flags, Some(flags));
        assert_eq!(settings.flags2, Some(flags2));
    }

    #[test]
    fn test_ch_layout_valid_layout() {
        // Safety: This is safe to call and the channel layout is allocated on the stack.
        let channel_layout = unsafe {
            AudioChannelLayout::wrap(ffmpeg_sys_next::AVChannelLayout {
                order: ffmpeg_sys_next::AVChannelOrder::AV_CHANNEL_ORDER_NATIVE,
                nb_channels: 2,
                u: ffmpeg_sys_next::AVChannelLayout__bindgen_ty_1 { mask: 0b11 },
                opaque: std::ptr::null_mut(),
            })
        };

        channel_layout.validate().expect("channel_layout is a valid value");
    }

    #[test]
    fn test_ch_layout_invalid_layout() {
        // Safety: This is safe to call and the channel layout is allocated on the stack.
        let channel_layout = unsafe {
            AudioChannelLayout::wrap(ffmpeg_sys_next::AVChannelLayout {
                order: ffmpeg_sys_next::AVChannelOrder::AV_CHANNEL_ORDER_UNSPEC,
                nb_channels: 0,
                u: ffmpeg_sys_next::AVChannelLayout__bindgen_ty_1 { mask: 0 },
                opaque: std::ptr::null_mut(),
            })
        };
        let result: Result<(), FfmpegError> = channel_layout.validate();
        assert_eq!(result.unwrap_err(), FfmpegError::Arguments("invalid channel layout"));
    }

    #[test]
    fn test_audio_encoder_settings_apply_error() {
        let settings = AudioEncoderSettings::builder()
            .sample_rate(0)
            .sample_fmt(AVSampleFormat::AV_SAMPLE_FMT_NONE)
            .ch_layout(AudioChannelLayout::new(2).expect("channel_count is a valid value"))
            .build();

        // Safety: We are zeroing the memory for the encoder context.
        let mut encoder = unsafe { std::mem::zeroed::<AVCodecContext>() };
        let result = settings.apply(&mut encoder);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            FfmpegError::Arguments("sample_rate, channel_layout and sample_fmt must be set")
        );
    }

    #[test]
    fn test_encoder_settings_apply_video() {
        let sample_aspect_ratio = AVRational { num: 1, den: 1 };
        let video_settings = VideoEncoderSettings::builder()
            .width(1920)
            .height(1080)
            .frame_rate(AVRational { num: 30, den: 1 })
            .pixel_format(AVPixelFormat::AV_PIX_FMT_YUV420P)
            .sample_aspect_ratio(sample_aspect_ratio)
            .gop_size(12)
            .build();

        // Safety: We are zeroing the memory for the encoder context.
        let mut encoder = unsafe { std::mem::zeroed::<AVCodecContext>() };
        let encoder_settings = EncoderSettings::Video(video_settings);
        let result = encoder_settings.apply(&mut encoder);

        assert!(result.is_ok(), "Failed to apply video settings: {:?}", result.err());
        assert_eq!(encoder.width, 1920);
        assert_eq!(encoder.height, 1080);
        assert_eq!(encoder.pix_fmt, AVPixelFormat::AV_PIX_FMT_YUV420P);
        assert_eq!(encoder.sample_aspect_ratio.num, 1);
        assert_eq!(encoder.sample_aspect_ratio.den, 1);
    }

    #[test]
    fn test_encoder_settings_apply_audio() {
        let audio_settings = AudioEncoderSettings::builder()
            .sample_rate(44100)
            .sample_fmt(AVSampleFormat::AV_SAMPLE_FMT_FLTP)
            .ch_layout(AudioChannelLayout::new(2).expect("channel_count is a valid value"))
            .thread_count(4)
            .build();

        // Safety: We are zeroing the memory for the encoder context.
        let mut encoder = unsafe { std::mem::zeroed::<AVCodecContext>() };
        let encoder_settings = EncoderSettings::Audio(audio_settings);
        let result = encoder_settings.apply(&mut encoder);

        assert!(result.is_ok(), "Failed to apply audio settings: {:?}", result.err());
        assert_eq!(encoder.sample_rate, 44100);
        assert_eq!(encoder.sample_fmt, AVSampleFormat::AV_SAMPLE_FMT_FLTP);
        assert_eq!(encoder.thread_count, 4);
    }

    #[test]
    fn test_encoder_settings_codec_specific_options() {
        let mut video_codec_options = Dictionary::new();
        video_codec_options.set(c"preset", c"fast").expect("Failed to set preset");

        let video_settings = VideoEncoderSettings::builder()
            .width(8)
            .height(8)
            .frame_rate(AVRational { num: 30, den: 1 })
            .pixel_format(AVPixelFormat::AV_PIX_FMT_YUV420P)
            .codec_specific_options(video_codec_options.clone())
            .build();
        let mut encoder_settings = EncoderSettings::Video(video_settings);
        let options = encoder_settings.codec_specific_options();

        assert!(options.is_some());
        assert_eq!(options.unwrap().get(c"preset"), Some(c"fast"));

        let mut audio_codec_options = Dictionary::new();
        audio_codec_options.set(c"bitrate", c"128k").expect("Failed to set bitrate");
        let audio_settings = AudioEncoderSettings::builder()
            .sample_rate(44100)
            .sample_fmt(AVSampleFormat::AV_SAMPLE_FMT_FLTP)
            .ch_layout(AudioChannelLayout::new(2).expect("channel_count is a valid value"))
            .thread_count(4)
            .codec_specific_options(audio_codec_options)
            .build();
        let mut encoder_settings = EncoderSettings::Audio(audio_settings);
        let options = encoder_settings.codec_specific_options();

        assert!(options.is_some());
        assert_eq!(options.unwrap().get(c"bitrate"), Some(c"128k"));
    }

    #[test]
    fn test_from_video_encoder_settings() {
        let sample_aspect_ratio = AVRational { num: 1, den: 1 };
        let video_settings = VideoEncoderSettings::builder()
            .width(1920)
            .height(1080)
            .frame_rate(AVRational { num: 30, den: 1 })
            .pixel_format(AVPixelFormat::AV_PIX_FMT_YUV420P)
            .sample_aspect_ratio(sample_aspect_ratio)
            .gop_size(12)
            .build();
        let encoder_settings: EncoderSettings = video_settings.into();

        if let EncoderSettings::Video(actual_video_settings) = encoder_settings {
            assert_eq!(actual_video_settings.width, 1920);
            assert_eq!(actual_video_settings.height, 1080);
            assert_eq!(actual_video_settings.frame_rate, AVRational { num: 30, den: 1 });
            assert_eq!(actual_video_settings.pixel_format, AVPixelFormat::AV_PIX_FMT_YUV420P);
            assert_eq!(actual_video_settings.sample_aspect_ratio, Some(sample_aspect_ratio));
            assert_eq!(actual_video_settings.gop_size, Some(12));
        } else {
            panic!("Expected EncoderSettings::Video variant");
        }
    }

    #[test]
    fn test_from_audio_encoder_settings() {
        let audio_settings = AudioEncoderSettings::builder()
            .sample_rate(44100)
            .sample_fmt(AVSampleFormat::AV_SAMPLE_FMT_FLTP)
            .ch_layout(AudioChannelLayout::new(2).expect("channel_count is a valid value"))
            .thread_count(4)
            .build();
        let encoder_settings: EncoderSettings = audio_settings.into();

        if let EncoderSettings::Audio(actual_audio_settings) = encoder_settings {
            assert_eq!(actual_audio_settings.sample_rate, 44100);
            assert_eq!(actual_audio_settings.sample_fmt, AVSampleFormat::AV_SAMPLE_FMT_FLTP);
            assert_eq!(actual_audio_settings.thread_count, Some(4));
        } else {
            panic!("Expected EncoderSettings::Audio variant");
        }
    }

    #[test]
    fn test_encoder_new_with_null_codec() {
        let codec = EncoderCodec::empty();
        let data = std::io::Cursor::new(Vec::new());
        let options = OutputOptions::builder().format_name("mp4").unwrap().build();
        let mut output = Output::new(data, options).expect("Failed to create Output");
        let incoming_time_base = AVRational { num: 1, den: 1000 };
        let outgoing_time_base = AVRational { num: 1, den: 1000 };
        let settings = VideoEncoderSettings::builder()
            .width(0)
            .height(0)
            .pixel_format(AVPixelFormat::AV_PIX_FMT_YUV420P)
            .frame_rate(AVRational { num: 0, den: 1 })
            .build();
        let result = Encoder::new(codec, &mut output, incoming_time_base, outgoing_time_base, settings);

        assert!(matches!(result, Err(FfmpegError::NoEncoder)));
    }

    #[test]
    fn test_encoder_new_success() {
        let codec = EncoderCodec::new(AV_CODEC_ID_MPEG4);
        assert!(codec.is_some(), "Failed to find MPEG-4 encoder");
        let data = std::io::Cursor::new(Vec::new());
        let options = OutputOptions::builder().format_name("mp4").unwrap().build();
        let mut output = Output::new(data, options).expect("Failed to create Output");
        let incoming_time_base = AVRational { num: 1, den: 1000 };
        let outgoing_time_base = AVRational { num: 1, den: 1000 };
        let settings = VideoEncoderSettings::builder()
            .width(1920)
            .height(1080)
            .frame_rate(AVRational { num: 30, den: 1 })
            .pixel_format(AVPixelFormat::AV_PIX_FMT_YUV420P)
            .build();
        let result = Encoder::new(codec.unwrap(), &mut output, incoming_time_base, outgoing_time_base, settings);

        assert!(result.is_ok(), "Encoder creation failed: {:?}", result.err());

        let encoder = result.unwrap();
        assert_eq!(encoder.incoming_time_base.num, 1);
        assert_eq!(encoder.incoming_time_base.den, 1000);
        assert_eq!(encoder.outgoing_time_base.num, 1);
        assert_eq!(encoder.outgoing_time_base.den, 1000);
        assert_eq!(encoder.stream_index, 0);
    }

    #[test]
    fn test_send_eof() {
        let codec = EncoderCodec::new(AV_CODEC_ID_MPEG4).expect("Failed to find MPEG-4 encoder");
        let data = std::io::Cursor::new(Vec::new());
        let options = OutputOptions::builder().format_name("mp4").unwrap().build();
        let mut output = Output::new(data, options).expect("Failed to create Output");
        let video_settings = VideoEncoderSettings::builder()
            .width(640)
            .height(480)
            .frame_rate(AVRational { num: 30, den: 1 })
            .pixel_format(AVPixelFormat::AV_PIX_FMT_YUV420P)
            .build();
        let mut encoder = Encoder::new(
            codec,
            &mut output,
            AVRational { num: 1, den: 1000 },
            AVRational { num: 1, den: 1000 },
            video_settings,
        )
        .expect("Failed to create encoder");

        let result = encoder.send_eof();
        assert!(result.is_ok(), "send_eof returned an error: {:?}", result.err());
        assert!(encoder.send_eof().is_err(), "send_eof should return an error");
    }

    #[test]
    fn test_encoder_getters() {
        let codec = EncoderCodec::new(AV_CODEC_ID_MPEG4).expect("Failed to find MPEG-4 encoder");
        let data = std::io::Cursor::new(Vec::new());
        let options = OutputOptions::builder().format_name("mp4").unwrap().build();
        let mut output = Output::new(data, options).expect("Failed to create Output");
        let incoming_time_base = AVRational { num: 1, den: 1000 };
        let outgoing_time_base = AVRational { num: 1, den: 1000 };
        let video_settings = VideoEncoderSettings::builder()
            .width(640)
            .height(480)
            .frame_rate(AVRational { num: 30, den: 1 })
            .pixel_format(AVPixelFormat::AV_PIX_FMT_YUV420P)
            .build();
        let encoder = Encoder::new(codec, &mut output, incoming_time_base, outgoing_time_base, video_settings)
            .expect("Failed to create encoder");

        let stream_index = encoder.stream_index();
        assert_eq!(stream_index, 0, "Unexpected stream index: expected 0, got {}", stream_index);

        let actual_incoming_time_base = encoder.incoming_time_base();
        assert_eq!(
            actual_incoming_time_base, incoming_time_base,
            "Unexpected incoming_time_base: expected {:?}, got {:?}",
            incoming_time_base, actual_incoming_time_base
        );

        let actual_outgoing_time_base = encoder.outgoing_time_base();
        assert_eq!(
            actual_outgoing_time_base, outgoing_time_base,
            "Unexpected outgoing_time_base: expected {:?}, got {:?}",
            outgoing_time_base, actual_outgoing_time_base
        );
    }

    #[test]
    fn test_encoder_encode_video() {
        let mut input = Input::open("../../assets/avc_aac.mp4").expect("Failed to open input file");
        let streams = input.streams();
        let video_stream = streams.best(AVMediaType::AVMEDIA_TYPE_VIDEO).expect("No video stream found");
        let mut decoder = Decoder::new(&video_stream)
            .expect("Failed to create decoder")
            .video()
            .expect("Failed to create video decoder");
        let mut output = Output::seekable(
            std::io::Cursor::new(Vec::new()),
            OutputOptions::builder().format_name("mp4").unwrap().build(),
        )
        .expect("Failed to create Output");
        let mut encoder = Encoder::new(
            EncoderCodec::new(AV_CODEC_ID_MPEG4).expect("Failed to find MPEG-4 encoder"),
            &mut output,
            AVRational { num: 1, den: 1000 },
            video_stream.time_base(),
            VideoEncoderSettings::builder()
                .width(decoder.width())
                .height(decoder.height())
                .frame_rate(decoder.frame_rate())
                .pixel_format(decoder.pixel_format())
                .build(),
        )
        .expect("Failed to create encoder");

        output.write_header().expect("Failed to write header");

        let input_stream_index = video_stream.index();

        while let Some(packet) = input.receive_packet().expect("Failed to receive packet") {
            if packet.stream_index() == input_stream_index {
                decoder.send_packet(&packet).expect("Failed to send packet");
                while let Some(frame) = decoder.receive_frame().expect("Failed to receive frame") {
                    encoder.send_frame(&frame).expect("Failed to send frame");
                    while let Some(packet) = encoder.receive_packet().expect("Failed to receive packet") {
                        output.write_packet(&packet).expect("Failed to write packet");
                    }
                }
            }
        }

        encoder.send_eof().expect("Failed to send EOF");
        while let Some(packet) = encoder.receive_packet().expect("Failed to receive packet") {
            output.write_packet(&packet).expect("Failed to write packet");
        }

        output.write_trailer().expect("Failed to write trailer");

        let mut cursor = std::io::Cursor::new(Bytes::from(output.into_inner().into_inner()));
        let mut boxes = Vec::new();
        while cursor.has_remaining() {
            let mut _box = scuffle_mp4::DynBox::demux(&mut cursor).expect("Failed to demux box");
            if let scuffle_mp4::DynBox::Mdat(mdat) = &mut _box {
                mdat.data.iter_mut().for_each(|buf| {
                    let mut hash = sha2::Sha256::new();
                    hash.write_all(buf).unwrap();
                    *buf = hash.finalize().to_vec().into();
                });
            }

            boxes.push(_box);
        }

        insta::assert_debug_snapshot!("test_encoder_encode_video", &boxes);
    }

    /// make sure [#248](https://github.com/ScuffleCloud/scuffle/pull/248) doesn't happen again
    #[test]
    fn test_pr_248() {
        let mut output = Output::seekable(
            std::io::Cursor::new(Vec::new()),
            OutputOptions::builder().format_name("mp4").unwrap().build(),
        )
        .expect("Failed to create Output");

        let mut settings = Dictionary::new();
        settings.set(c"key", c"value").expect("Failed to set Dictionary entry");

        let codec = EncoderCodec::new(AV_CODEC_ID_MPEG4).expect("Missing MPEG-4 codec");

        Encoder::new(
            codec,
            &mut output,
            AVRational { num: 1, den: 100 },
            AVRational { num: 1, den: 100 },
            VideoEncoderSettings::builder()
                .width(16)
                .height(16)
                .frame_rate(AVRational { num: 30, den: 1 })
                .pixel_format(AVPixelFormat::AV_PIX_FMT_YUV420P)
                .codec_specific_options(settings)
                .build(),
        )
        .expect("Failed to create new Encoder");
    }
}
