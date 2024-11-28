use anyhow::Context;
use mp4::codec::AudioCodec;
use pb::scuffle::video::v1::types::AudioConfig;
use scuffle_ffmpeg::codec::EncoderCodec;
use scuffle_ffmpeg::dict::Dictionary;
use scuffle_ffmpeg::encoder::{AudioEncoderSettings, MuxerEncoder, MuxerSettings};
use scuffle_ffmpeg::error::FfmpegError;
use scuffle_ffmpeg::ffi::{AVCodecID, AVPictureType};
use scuffle_ffmpeg::io::channel::ChannelCompatSend;
use scuffle_ffmpeg::io::OutputOptions;
use scuffle_ffmpeg::packet::Packet;
use tokio::sync::mpsc;

use super::{muxer_options, Transcoder};

pub fn codec_options(codec: AudioCodec) -> anyhow::Result<(EncoderCodec, Dictionary)> {
	Ok(match codec {
		AudioCodec::Aac { object_type } => {
			let codec = scuffle_ffmpeg::codec::EncoderCodec::by_name("libfdk_aac")
				.or_else(|| scuffle_ffmpeg::codec::EncoderCodec::new(AVCodecID::AV_CODEC_ID_AAC))
				.ok_or(FfmpegError::NoEncoder)
				.context("failed to find aac encoder")?;

			(
				codec,
				Dictionary::builder()
					.set(
						"profile",
						match object_type {
							aac::AudioObjectType::AacLowComplexity => "aac_low",
							aac::AudioObjectType::AacMain => "aac_main",
							aac::AudioObjectType::Unknown(profile) => {
								anyhow::bail!("invalid aac profile: {profile}");
							}
						},
					)
					.build(),
			)
		}
		AudioCodec::Opus => {
			let codec = scuffle_ffmpeg::codec::EncoderCodec::by_name("libopus")
				.or_else(|| scuffle_ffmpeg::codec::EncoderCodec::new(AVCodecID::AV_CODEC_ID_OPUS))
				.ok_or(FfmpegError::NoEncoder)
				.context("failed to find opus encoder")?;

			(codec, Dictionary::new())
		}
	})
}

impl Transcoder {
	pub fn setup_audio_encoder(
		&mut self,
		sender: mpsc::Sender<Vec<u8>>,
		audio_config: &AudioConfig,
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

		self.audio_encoders.push(MuxerEncoder::new(
			encoder_codec,
			output,
			self.audio_decoder.as_ref().unwrap().time_base(),
			self.audio_decoder.as_ref().unwrap().time_base(),
			AudioEncoderSettings::builder(
				audio_config.sample_rate,
				self.audio_decoder.as_ref().unwrap().channel_layout(),
				self.audio_decoder.as_ref().unwrap().channels(),
				self.audio_decoder.as_ref().unwrap().sample_format(),
			)
			.bitrate(audio_config.bitrate)
			.rc_max_rate(audio_config.bitrate)
			.rc_buffer_size(audio_config.bitrate as i32 * 2)
			.thread_count(1)
			.codec_specific_options(encoder_options)
			.build(),
			MuxerSettings::builder()
				.interleave(true)
				.muxer_options(muxer_options())
				.build(),
		)?);

		Ok(())
	}

	pub fn handle_audio_packet(&mut self, mut packet: Packet) -> anyhow::Result<()> {
		packet.set_pos(Some(-1));

		for copy in self.audio_copies.iter_mut() {
			copy.write_interleaved_packet(packet.clone()).context("copy")?;
		}

		if let Some(decoder) = &mut self.audio_decoder {
			decoder.send_packet(&packet).context("decoder send packet")?;
		}

		self.handle_audio_decoder().context("decoder")?;

		Ok(())
	}

	pub fn handle_audio_eof(&mut self) -> anyhow::Result<()> {
		for copy in self.audio_copies.iter_mut() {
			copy.write_trailer().context("copy")?;
		}

		if let Some(decoder) = &mut self.audio_decoder {
			decoder.send_eof().context("decoder eof")?;
		}

		self.handle_audio_decoder()?;

		for encoder in self.audio_encoders.iter_mut() {
			encoder.send_eof().context("encoder eof")?;
		}

		Ok(())
	}

	fn handle_audio_decoder(&mut self) -> anyhow::Result<()> {
		if let Some(decoder) = self.audio_decoder.as_mut() {
			while let Some(mut frame) = decoder.receive_frame().context("receive frame")? {
				frame.set_pict_type(AVPictureType::AV_PICTURE_TYPE_NONE);
				let frame_timestamp = frame.best_effort_timestamp();
				frame.set_pts(frame_timestamp);

				for encoder in self.audio_encoders.iter_mut() {
					encoder.send_frame(&frame).context("encoder")?;
				}
			}
		}

		Ok(())
	}
}
