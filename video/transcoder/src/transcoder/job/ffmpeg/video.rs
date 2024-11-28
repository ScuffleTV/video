use anyhow::Context;
use mp4::codec::VideoCodec;
use pb::scuffle::video::v1::types::VideoConfig;
use scuffle_ffmpeg::codec::EncoderCodec;
use scuffle_ffmpeg::dict::Dictionary;
use scuffle_ffmpeg::encoder::{MuxerEncoder, MuxerSettings, VideoEncoderSettings};
use scuffle_ffmpeg::error::FfmpegError;
use scuffle_ffmpeg::ffi::{AVCodecID, AVPictureType, AVRational};
use scuffle_ffmpeg::io::channel::ChannelCompatSend;
use scuffle_ffmpeg::io::OutputOptions;
use tokio::sync::mpsc;

use super::{muxer_options, Limiter, Scalar, Transcoder};
use crate::config::TranscoderConfig;

pub fn codec_options(config: &TranscoderConfig, codec: VideoCodec) -> anyhow::Result<(EncoderCodec, Dictionary)> {
	match codec {
		VideoCodec::Avc { level, profile, .. } => {
			let mut options = Dictionary::from(config.h264_encoder_options.clone());

			options
				.set(
					"profile",
					match profile {
						66 => "baseline",
						77 => "main",
						100 => "high",
						_ => {
							anyhow::bail!("invalid h264 profile: {profile}");
						}
					},
				)
				.context("failed to set h264 profile")?;

			options
				.set(
					"level",
					match level {
						30 => "3.0",
						31 => "3.1",
						32 => "3.2",
						40 => "4.0",
						41 => "4.1",
						42 => "4.2",
						50 => "5.0",
						51 => "5.1",
						52 => "5.2",
						60 => "6.0",
						61 => "6.1",
						62 => "6.2",
						_ => {
							anyhow::bail!("invalid avc level: {level}");
						}
					},
				)
				.context("failed to set h264 level")?;

			Ok((
				config
					.h264_encoder
					.as_ref()
					.map(|name| scuffle_ffmpeg::codec::EncoderCodec::by_name(name))
					.unwrap_or_else(|| scuffle_ffmpeg::codec::EncoderCodec::new(AVCodecID::AV_CODEC_ID_H264))
					.ok_or(FfmpegError::NoEncoder)
					.context("failed to find h264 encoder")?,
				options,
			))
		}
		VideoCodec::Av1 { .. } => {
			anyhow::bail!("av1 encoding is not supported");
		}
		VideoCodec::Hevc { .. } => {
			anyhow::bail!("hevc encoding is not supported");
		}
	}
}

impl Transcoder {
	pub fn setup_video_encoder(
		&mut self,
		sender: mpsc::Sender<Vec<u8>>,
		video_config: &VideoConfig,
		encoder_codec: EncoderCodec,
		encoder_options: Dictionary,
	) -> anyhow::Result<()> {
		let output = scuffle_ffmpeg::io::Output::new(
			sender.into_compat(),
			OutputOptions {
				format_name: Some("mp4"),
				..Default::default()
			},
		)
		.context("failed to create output")?;

		self.frame_limiters
			.push(Limiter::new(video_config.fps, self.video_decoder.time_base()));

		self.video_scalars.push(Scalar::new(
			self.video_scalars
				.last()
				.map(|s| s.width())
				.unwrap_or_else(|| self.video_decoder.width()),
			self.video_scalars
				.last()
				.map(|s| s.height())
				.unwrap_or_else(|| self.video_decoder.height()),
			self.video_scalars
				.last()
				.map(|s| s.pixel_format())
				.unwrap_or_else(|| self.video_decoder.pixel_format()),
			video_config.width,
			video_config.height,
			self.video_decoder.pixel_format(),
		)?);

		self.video_encoders.push(MuxerEncoder::new(
			encoder_codec,
			output,
			self.video_decoder.time_base(),
			AVRational {
				num: 1,
				den: 1000 * video_config.fps,
			},
			VideoEncoderSettings::builder(
				video_config.width,
				video_config.height,
				video_config.fps,
				self.video_decoder.pixel_format(),
			)
			.bitrate(video_config.bitrate)
			.rc_max_rate(video_config.bitrate)
			.rc_buffer_size(video_config.bitrate as i32 * 2)
			.gop_size(video_config.fps * 2)
			.max_b_frames(0)
			.thread_count(0)
			.codec_specific_options(encoder_options)
			.build(),
			MuxerSettings::builder()
				.interleave(true)
				.muxer_options(muxer_options())
				.build(),
		)?);

		Ok(())
	}

	pub fn handle_video_packet(&mut self, mut packet: scuffle_ffmpeg::packet::Packet) -> anyhow::Result<()> {
		packet.set_pos(Some(-1));
		for copy in self.video_copies.iter_mut() {
			copy.write_interleaved_packet(packet.clone()).context("copy")?;
		}

		if packet.is_key() || !self.video_encoders.is_empty() {
			self.video_decoder.send_packet(&packet).context("decoder send")?;
		}

		self.handle_video_decoder().context("decoder")?;

		Ok(())
	}

	pub fn handle_video_eof(&mut self) -> anyhow::Result<()> {
		for copy in self.video_copies.iter_mut() {
			copy.write_trailer().context("copy")?;
		}

		self.video_decoder.send_eof().context("decoder eof")?;

		self.handle_video_decoder().context("decoder")?;

		for encoder in self.video_encoders.iter_mut() {
			encoder.send_eof().context("encoder eof")?;
		}

		Ok(())
	}

	fn handle_video_decoder(&mut self) -> anyhow::Result<()> {
		while let Some(mut frame) = self.video_decoder.receive_frame().context("receive frame")? {
			frame.set_pict_type(AVPictureType::AV_PICTURE_TYPE_NONE);
			let frame_timestamp = frame.best_effort_timestamp();
			frame.set_pts(frame_timestamp);
			frame.set_format(self.video_decoder.pixel_format() as i32);

			if self.last_screenshot.elapsed() > self.screenshot_interval {
				let mut frame = self.screenshot_scalar.process(&frame).context("screenshot")?.clone();
				frame.set_time_base(self.video_decoder.time_base());
				self.screenshot_output.blocking_send(frame.0).context("screenshot")?;
				self.last_screenshot = std::time::Instant::now();
			}

			let mut frames = vec![];
			for (scalar, limiter) in self.video_scalars.iter_mut().zip(self.frame_limiters.iter_mut()) {
				if !limiter.limit(&frame) {
					break;
				}

				frames.push(scalar.process(frames.last().copied().unwrap_or(&frame.0)).context("scalar")?);
			}

			for (encoder, frame) in self.video_encoders.iter_mut().zip(frames) {
				encoder.send_frame(frame).context("encoder")?;
			}
		}

		Ok(())
	}
}
