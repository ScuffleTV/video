use std::borrow::Cow;

use imgref::Img;
use rgb::RGBA8;
use scuffle_image_processor_proto::Task;

use super::{Decoder, DecoderError, DecoderFrontend, DecoderInfo, LoopCount};
use crate::worker::process::frame::{Frame, FrameRef};

pub struct FfmpegDecoder<'data> {
	input: scuffle_ffmpeg::io::Input<std::io::Cursor<Cow<'data, [u8]>>>,
	decoder: scuffle_ffmpeg::decoder::VideoDecoder,
	scaler: scuffle_ffmpeg::scalar::Scalar,
	info: DecoderInfo,
	input_stream_index: i32,
	average_frame_duration_ts: u64,
	duration_ms: i64,
	previous_timestamp_ts: Option<u64>,
	send_packet: bool,
	eof: bool,
	done: bool,
	frame: Frame,
}

const fn cast_bytes_to_rgba(bytes: &[u8]) -> &[rgb::RGBA8] {
	unsafe { std::slice::from_raw_parts(bytes.as_ptr() as *const _, bytes.len() / 4) }
}

static FFMPEG_LOGGING_INITIALIZED: std::sync::Once = std::sync::Once::new();

impl<'data> FfmpegDecoder<'data> {
	#[tracing::instrument(skip_all, fields(name = "FfmpegDecoder::new"))]
	pub fn new(task: &Task, data: Cow<'data, [u8]>) -> Result<Self, DecoderError> {
		FFMPEG_LOGGING_INITIALIZED.call_once(|| {
			scuffle_ffmpeg::log::log_callback_tracing();
		});

		let input = scuffle_ffmpeg::io::Input::seekable(std::io::Cursor::new(data))?;

		let input_stream = input
			.streams()
			.best(scuffle_ffmpeg::ffi::AVMediaType::AVMEDIA_TYPE_VIDEO)
			.ok_or(DecoderError::NoVideoStream)?;

		let input_stream_index = input_stream.index();

		let input_stream_time_base = input_stream.time_base();
		let input_stream_duration_ts = input_stream.duration().unwrap_or(0);
		let input_stream_frames = input_stream.nb_frames().ok_or(DecoderError::NoFrameCount)?.max(1);

		if input_stream_time_base.den == 0 || input_stream_time_base.num == 0 {
			return Err(DecoderError::InvalidTimeBase);
		}

		let decoder = match scuffle_ffmpeg::decoder::Decoder::new(&input_stream)? {
			scuffle_ffmpeg::decoder::Decoder::Video(decoder) => decoder,
			_ => {
				return Err(DecoderError::InvalidVideoDecoder);
			}
		};

		if let Some(max_input_width) = task.limits.as_ref().and_then(|l| l.max_input_width) {
			if decoder.width() > max_input_width as i32 {
				return Err(DecoderError::TooWide(decoder.width()));
			}
		}

		if let Some(max_input_height) = task.limits.as_ref().and_then(|l| l.max_input_height) {
			if decoder.height() > max_input_height as i32 {
				return Err(DecoderError::TooHigh(decoder.height()));
			}
		}

		if let Some(max_input_frame_count) = task.limits.as_ref().and_then(|l| l.max_input_frame_count) {
			if input_stream_frames > max_input_frame_count as i64 {
				return Err(DecoderError::TooManyFrames(input_stream_frames));
			}
		}

		let duration_ms =
			(input_stream_duration_ts * input_stream_time_base.num as i64 * 1000) / input_stream_time_base.den as i64;

		if duration_ms < 0 {
			return Err(DecoderError::InvalidTimeBase);
		}

		if let Some(max_input_duration_ms) = task.limits.as_ref().and_then(|l| l.max_input_duration_ms) {
			// actual duration
			// = duration * (time_base.num / time_base.den) * 1000
			// = (duration * time_base.num * 1000) / time_base.den

			if duration_ms > max_input_duration_ms as i64 {
				return Err(DecoderError::TooLong(duration_ms));
			}
		}

		if decoder.pixel_format() == scuffle_ffmpeg::ffi::AVPixelFormat::AV_PIX_FMT_NONE {
			return Err(DecoderError::MalformedInput);
		}

		let scaler = scuffle_ffmpeg::scalar::Scalar::new(
			decoder.width(),
			decoder.height(),
			decoder.pixel_format(),
			decoder.width(),
			decoder.height(),
			scuffle_ffmpeg::ffi::AVPixelFormat::AV_PIX_FMT_RGBA,
		)?;

		let info = DecoderInfo {
			width: decoder.width() as usize,
			height: decoder.height() as usize,
			frame_count: input_stream_frames as usize,
			// TODO: Support loop count from ffmpeg.
			loop_count: LoopCount::Infinite,
			// TODO: This is a rough estimate of the timescale. We store it as a single integer but perhaps we should,
			// store it as a fraction instead. Would require changes throughout the codebase.
			timescale: (input_stream_time_base.den as f64 / input_stream_time_base.num as f64).round() as u64,
		};

		let average_frame_duration_ts = (input_stream_duration_ts / input_stream_frames) as u64;

		let frame = Frame {
			image: Img::new(vec![RGBA8::default(); info.width * info.height], info.width, info.height),
			duration_ts: average_frame_duration_ts,
		};

		Ok(Self {
			info,
			input,
			scaler,
			decoder,
			input_stream_index,
			done: false,
			eof: false,
			send_packet: true,
			frame,
			average_frame_duration_ts,
			duration_ms,
			previous_timestamp_ts: Some(0),
		})
	}
}

impl Decoder for FfmpegDecoder<'_> {
	fn backend(&self) -> DecoderFrontend {
		DecoderFrontend::Ffmpeg
	}

	#[tracing::instrument(skip_all, fields(name = "FfmpegDecoder::decode"))]
	fn decode(&mut self) -> Result<Option<FrameRef>, DecoderError> {
		if self.done {
			return Ok(None);
		}

		loop {
			if self.send_packet && !self.eof {
				let packet = self
					.input
					.packets()
					.find_map(|packet| match packet {
						Ok(packet) => {
							if packet.stream_index() == self.input_stream_index {
								Some(Ok(packet))
							} else {
								None
							}
						}
						Err(err) => {
							self.done = true;
							Some(Err(err))
						}
					})
					.transpose()?;

				if let Some(packet) = packet {
					self.decoder.send_packet(&packet).inspect_err(|_| {
						self.done = true;
					})?;
				} else {
					self.decoder.send_eof().inspect_err(|_| {
						self.done = true;
					})?;
					self.eof = true;
				}

				self.send_packet = false;
			}

			let frame = self.decoder.receive_frame().inspect_err(|_| {
				self.done = true;
			})?;

			if let Some(frame) = frame {
				let frame = self.scaler.process(&frame).inspect_err(|_| {
					self.done = true;
				})?;

				// The frame has padding, so we need to copy the data.
				let frame_data = frame.data(0).unwrap();
				let frame_linesize = frame.linesize(0).unwrap();

				if frame_linesize == frame.width() as i32 * 4 {
					// No padding, so we can just copy the data.
					self.frame.image.buf_mut().copy_from_slice(cast_bytes_to_rgba(frame_data));
				} else {
					// The frame has padding, so we need to copy the data.
					for (i, row) in self.frame.image.buf_mut().chunks_exact_mut(frame.width()).enumerate() {
						let row_data = &frame_data[i * frame_linesize as usize..][..frame.width() * 4];
						row.copy_from_slice(cast_bytes_to_rgba(row_data));
					}
				}

				fn map_num(num: i64) -> Option<u64> {
					if num < 0 {
						None
					} else {
						Some(num as u64)
					}
				}

				let best_effort_ts = frame.best_effort_timestamp().and_then(map_num);

				self.frame.duration_ts = frame
					.duration()
					.and_then(map_num)
					.or_else(|| {
						let current_ts = best_effort_ts?;
						let previous_ts = self.previous_timestamp_ts?;
						current_ts.checked_sub(previous_ts)
					})
					.unwrap_or(self.average_frame_duration_ts);

				self.previous_timestamp_ts = best_effort_ts;

				return Ok(Some(self.frame.as_ref()));
			} else if self.eof {
				self.done = true;
				return Ok(None);
			} else {
				self.send_packet = true;
			}
		}
	}

	fn duration_ms(&self) -> i64 {
		self.duration_ms
	}

	fn info(&self) -> DecoderInfo {
		self.info
	}
}
