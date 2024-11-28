use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Context;
use bytes::Bytes;
use pb::scuffle::video::v1::types::{AudioConfig, VideoConfig};
use scuffle_ffmpeg::decoder::Decoder;
use scuffle_ffmpeg::dict::Dictionary;
use scuffle_ffmpeg::error::FfmpegError;
use scuffle_ffmpeg::ffi::{AVMediaType, AVPixelFormat};
use scuffle_ffmpeg::frame::Frame;
use scuffle_ffmpeg::io::channel::{ChannelCompatRecv as _, ChannelCompatSend as _};
use scuffle_ffmpeg::io::OutputOptions;
use scuffle_ffmpeg::log::LogLevel;
use tokio::sync::mpsc;
use video_common::database::Rendition;

use crate::global::TranscoderGlobal;

mod audio;
mod video;

const MP4_FLAGS: &str = "frag_keyframe+frag_every_frame+empty_moov+delay_moov+default_base_moof";

type ChannelCompatRecv = scuffle_ffmpeg::io::channel::ChannelCompat<mpsc::Receiver<Bytes>>;
type ChannelCompatSend = scuffle_ffmpeg::io::channel::ChannelCompat<mpsc::Sender<Vec<u8>>>;

type Input = scuffle_ffmpeg::io::Input<ChannelCompatRecv>;
type Output = scuffle_ffmpeg::io::Output<ChannelCompatSend>;
type VideoDecoder = scuffle_ffmpeg::decoder::VideoDecoder;
type AudioDecoder = scuffle_ffmpeg::decoder::AudioDecoder;
type Encoder = scuffle_ffmpeg::encoder::MuxerEncoder<ChannelCompatSend>;
type Scalar = scuffle_ffmpeg::scalar::Scalar;
type Limiter = scuffle_ffmpeg::limiter::FrameRateLimiter;

static SETUP_LOGGING: std::sync::Once = std::sync::Once::new();

fn muxer_options() -> Dictionary {
	Dictionary::builder().set("movflags", MP4_FLAGS).build()
}

pub fn screenshot_size(width: i32, height: i32) -> (i32, i32) {
	const MAX_SCREENSHOT_SIZE: i32 = 180;

	let aspect_ratio = width as f64 / height as f64;

	let (width, height) = if aspect_ratio > 1.0 && height > MAX_SCREENSHOT_SIZE {
		(
			(MAX_SCREENSHOT_SIZE as f64 * aspect_ratio).round() as i32,
			MAX_SCREENSHOT_SIZE,
		)
	} else if aspect_ratio < 1.0 && width > MAX_SCREENSHOT_SIZE {
		(
			MAX_SCREENSHOT_SIZE,
			(MAX_SCREENSHOT_SIZE as f64 / aspect_ratio).round() as i32,
		)
	} else {
		(width, height)
	};

	(width, height)
}

pub struct Transcoder {
	input: Input,
	video_stream_index: i32,
	audio_stream_index: i32,
	video_decoder: VideoDecoder,
	audio_decoder: Option<AudioDecoder>,
	video_copies: Vec<Output>,
	audio_copies: Vec<Output>,
	video_scalars: Vec<Scalar>,
	frame_limiters: Vec<Limiter>,
	video_encoders: Vec<Encoder>,
	audio_encoders: Vec<Encoder>,
	last_screenshot: Instant,
	screenshot_interval: Duration,
	screenshot_scalar: Scalar,
	screenshot_output: mpsc::Sender<Frame>,
}

impl Transcoder {
	pub fn new(
		global: &Arc<impl TranscoderGlobal>,
		input: mpsc::Receiver<Bytes>,
		screenshot_output: mpsc::Sender<Frame>,
		mut outputs: HashMap<Rendition, mpsc::Sender<Vec<u8>>>,
		mut video_configs: Vec<VideoConfig>,
		mut audio_outputs: Vec<AudioConfig>,
	) -> anyhow::Result<Self> {
		SETUP_LOGGING.call_once(|| {
			scuffle_ffmpeg::log::set_log_level(LogLevel::Trace);
			scuffle_ffmpeg::log::log_callback_tracing();
		});

		let input = scuffle_ffmpeg::io::Input::new(input.into_compat()).context("failed to create input")?;

		let video_stream = input
			.streams()
			.best(AVMediaType::AVMEDIA_TYPE_VIDEO)
			.ok_or(FfmpegError::NoStream)
			.context("failed to find video stream")?;

		let audio_stream = input
			.streams()
			.best(AVMediaType::AVMEDIA_TYPE_AUDIO)
			.ok_or(FfmpegError::NoStream)
			.context("failed to find video stream")?;

		let video_decoder =
			match scuffle_ffmpeg::decoder::Decoder::new(&video_stream).context("failed to create h264 decoder")? {
				Decoder::Video(decoder) => decoder,
				_ => anyhow::bail!("expected video decoder"),
			};

		let (screenshot_width, screenshot_height) = screenshot_size(video_decoder.width(), video_decoder.height());

		let screenshot_scalar = scuffle_ffmpeg::scalar::Scalar::new(
			video_decoder.width(),
			video_decoder.height(),
			video_decoder.pixel_format(),
			screenshot_width,
			screenshot_height,
			AVPixelFormat::AV_PIX_FMT_RGBA,
		)
		.context("failed to create screenshot scalar")?;

		let mut this = Self {
			audio_stream_index: audio_stream.index(),
			video_stream_index: video_stream.index(),
			video_decoder,
			input,
			last_screenshot: Instant::now() - global.config().screenshot_interval,
			screenshot_interval: global.config().screenshot_interval,
			audio_decoder: None,
			video_copies: Vec::new(),
			audio_copies: Vec::new(),
			video_scalars: Vec::new(),
			frame_limiters: Vec::new(),
			video_encoders: Vec::new(),
			audio_encoders: Vec::new(),
			screenshot_output,
			screenshot_scalar,
		};

		if audio_outputs.iter().any(|c| c.rendition() == Rendition::AudioSource.into()) {
			let sender = outputs
				.remove(&Rendition::AudioSource)
				.ok_or_else(|| anyhow::anyhow!("missing audio source output"))?;

			let mut output = scuffle_ffmpeg::io::Output::new(
				sender.into_compat(),
				OutputOptions {
					format_name: Some("mp4"),
					..Default::default()
				},
			)
			.context("failed to create output")?;

			let audio_stream = this
				.input
				.streams()
				.best(AVMediaType::AVMEDIA_TYPE_AUDIO)
				.ok_or(FfmpegError::NoStream)
				.context("failed to find video stream")?;

			output.copy_stream(&audio_stream).context("failed to copy audio stream")?;
			output.write_header_with_options(&mut muxer_options())?;

			this.audio_copies.push(output);
		}

		if video_configs.iter().any(|c| c.rendition() == Rendition::VideoSource.into()) {
			let sender = outputs
				.remove(&Rendition::VideoSource)
				.ok_or_else(|| anyhow::anyhow!("missing video source output"))?;

			let mut output = scuffle_ffmpeg::io::Output::new(
				sender.into_compat(),
				OutputOptions {
					format_name: Some("mp4"),
					..Default::default()
				},
			)
			.context("failed to create output")?;

			let video_stream = this
				.input
				.streams()
				.best(AVMediaType::AVMEDIA_TYPE_VIDEO)
				.ok_or(FfmpegError::NoStream)
				.context("failed to find video stream")?;

			output.copy_stream(&video_stream).context("failed to copy video stream")?;
			output.write_header_with_options(&mut muxer_options())?;

			this.video_copies.push(output);
		}

		audio_outputs.retain(|c| c.rendition() != Rendition::AudioSource.into());
		video_configs.retain(|c| c.rendition() != Rendition::VideoSource.into());

		let config = global.config();

		if !video_configs.is_empty() {
			for video_config in video_configs {
				let codec = video_config
					.codec
					.parse()
					.map_err(|err| anyhow::anyhow!("failed to parse video codec: {err}"))?;

				let (encoder_codec, encoder_options) = video::codec_options(config, codec)?;

				let sender = outputs
					.remove(&Rendition::from(video_config.rendition()))
					.ok_or_else(|| anyhow::anyhow!("missing video output"))?;
				this.setup_video_encoder(sender, &video_config, encoder_codec, encoder_options)?;
			}
		}

		if !audio_outputs.is_empty() {
			let audio_stream = this
				.input
				.streams()
				.best(AVMediaType::AVMEDIA_TYPE_AUDIO)
				.ok_or(FfmpegError::NoStream)
				.context("failed to find video stream")?;

			this.audio_decoder = Some(
				match scuffle_ffmpeg::decoder::Decoder::new(&audio_stream).context("failed to create aac decoder")? {
					Decoder::Audio(decoder) => decoder,
					_ => anyhow::bail!("expected audio decoder"),
				},
			);

			for audio_config in audio_outputs {
				let codec = audio_config
					.codec
					.parse()
					.map_err(|err| anyhow::anyhow!("failed to parse audio codec: {err}"))?;

				let (encoder_codec, encoder_options) = audio::codec_options(codec)?;

				let sender = outputs
					.remove(&Rendition::from(audio_config.rendition()))
					.ok_or_else(|| anyhow::anyhow!("missing audio output"))?;
				this.setup_audio_encoder(sender, &audio_config, encoder_codec, encoder_options)?;
			}
		}

		if !outputs.is_empty() {
			anyhow::bail!("missing outputs: {:?}", outputs.keys());
		}

		Ok(this)
	}

	pub fn run(&mut self) -> anyhow::Result<()> {
		while let Some(mut packet) = self.input.receive_packet().context("receive packet")? {
			let stream_idx = packet.stream_index();
			packet.set_stream_index(0);

			if stream_idx == self.video_stream_index {
				self.handle_video_packet(packet).context("video")?;
			} else if stream_idx == self.audio_stream_index {
				self.handle_audio_packet(packet).context("audio")?;
			}
		}

		self.handle_audio_eof().context("audio eof")?;

		self.handle_video_eof().context("video eof")?;

		Ok(())
	}
}
