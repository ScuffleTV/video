use std::borrow::Cow;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use bytes::Bytes;
use file_format::FileFormat;
use scuffle_image_processor_proto::{animation_config, Output, OutputFormat, OutputFormatOptions, Task};
use tokio::sync::OwnedSemaphorePermit;

use super::decoder::{AnyDecoder, Decoder, DecoderFrontend, DecoderInfo, LoopCount};
use super::encoder::{AnyEncoder, Encoder, EncoderBackend, EncoderError, EncoderSettings};
use super::resize::{ImageResizer, ResizeOutputTarget};
use super::JobError;

pub struct JobOutput {
	pub input: InputImage,
	pub output: Vec<OutputImage>,
}

pub struct OutputImage {
	pub format: OutputFormat,
	pub format_name: Option<String>,
	pub format_idx: usize,
	pub resize_idx: usize,
	pub scale: Option<usize>,
	pub width: usize,
	pub height: usize,
	pub data: Vec<u8>,
	pub frame_count: usize,
	pub duration_ms: u64,
	pub loop_count: LoopCount,
}

pub struct InputImage {
	pub format: FileFormat,
	pub width: usize,
	pub height: usize,
	pub loop_count: LoopCount,
	pub duration_ms: u64,
	pub frame_count: usize,
}

#[derive(Clone, Copy)]
enum FrameConfig {
	Skip,
	DurationMs(u32),
}

#[derive(Clone)]
struct CancelToken {
	cancelled: Arc<AtomicBool>,
}

impl CancelToken {
	fn new() -> Self {
		Self {
			cancelled: Arc::new(AtomicBool::new(false)),
		}
	}

	fn is_cancelled(&self) -> bool {
		self.cancelled.load(std::sync::atomic::Ordering::Relaxed)
	}
}

impl Drop for CancelToken {
	fn drop(&mut self) {
		self.cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
	}
}

pub async fn spawn(task: Task, input: Bytes, permit: Arc<OwnedSemaphorePermit>) -> Result<JobOutput, JobError> {
	let cancel_token = CancelToken::new();
	let _cancel_guard = cancel_token.clone();

	let span = tracing::Span::current();

	tokio::task::spawn_blocking(move || {
		// This prevents the permit from being dropped before the task is finished.
		// This is because there is no way to cancel a blocking task.
		// So, if we cancel the parent future we need to make sure the permit is still
		// held, as we are still technically running. If we dont do this we might use
		// too many system resources.
		let _span = span.enter();
		let _permit = permit;
		let mut task = BlockingTask::new(&task, &input)?;

		while task.drive()? {
			// Check if the task has been cancelled.
			if cancel_token.is_cancelled() {
				return Err(JobError::Internal("cancelled"));
			}
		}

		task.finish()
	})
	.await?
}

struct BlockingTask<'a> {
	decoder: AnyDecoder<'a>,
	decoder_info: DecoderInfo,
	input: InputImage,
	frame_configs: Vec<Option<FrameConfig>>,
	resizer: ImageResizer,
	static_encoders: Vec<(usize, Vec<(ResizeOutputTarget, AnyEncoder)>)>,
	anim_encoders: Vec<(usize, Vec<(ResizeOutputTarget, AnyEncoder)>)>,
	static_frame_idx: usize,
	frame_idx: usize,
	duration_carried_ms: f64,
	frame_rate_factor: Option<f64>,
	loop_count: LoopCount,
}

type IdxFormatOptions<'a> = (usize, &'a OutputFormatOptions);

fn split_formats(output: &Output) -> (Vec<IdxFormatOptions>, Vec<IdxFormatOptions>) {
	output
		.formats
		.iter()
		.enumerate()
		.fold((Vec::new(), Vec::new()), |mut acc, (idx, format_options)| {
			match format_options.format() {
				OutputFormat::AvifStatic | OutputFormat::WebpStatic | OutputFormat::PngStatic => {
					acc.0.push((idx, format_options))
				}
				OutputFormat::AvifAnim | OutputFormat::GifAnim | OutputFormat::WebpAnim => acc.1.push((idx, format_options)),
			}
			acc
		})
}

fn build_encoder_set(
	format_options: &OutputFormatOptions,
	resize_outputs: &[ResizeOutputTarget],
	loop_count: LoopCount,
) -> Result<Vec<(ResizeOutputTarget, AnyEncoder)>, JobError> {
	let encoder_frontend = match format_options.format() {
		OutputFormat::AvifStatic | OutputFormat::AvifAnim => EncoderBackend::LibAvif,
		OutputFormat::PngStatic => EncoderBackend::Png,
		OutputFormat::WebpStatic | OutputFormat::WebpAnim => EncoderBackend::LibWebp,
		OutputFormat::GifAnim => EncoderBackend::Gifski,
	};

	resize_outputs
		.iter()
		.map(|target| {
			Ok((
				*target,
				encoder_frontend.build(EncoderSettings {
					loop_count,
					format: format_options.format(),
					name: format_options.name.clone(),
					quality: format_options.quality(),
					static_image: matches!(
						format_options.format(),
						OutputFormat::AvifStatic | OutputFormat::WebpStatic | OutputFormat::PngStatic
					),
					timescale: 1000, // millisecond timescale
				})?,
			))
		})
		.collect::<Result<Vec<_>, JobError>>()
}

impl<'a> BlockingTask<'a> {
	fn new(task: &'a Task, input: &'a [u8]) -> Result<Self, JobError> {
		let output = task.output.as_ref().ok_or(JobError::InvalidJob)?;
		let anim_config = output.animation_config.as_ref();

		let (static_formats, anim_formats) = split_formats(output);

		if static_formats.is_empty() && anim_formats.is_empty() {
			return Err(JobError::InvalidJob);
		}

		let file_format = FileFormat::from_bytes(input);
		let decoder = DecoderFrontend::from_format(file_format)?.build(task, Cow::Borrowed(input))?;

		let decoder_info = decoder.info();

		if let Some(metadata) = task.input.as_ref().and_then(|input| input.metadata.as_ref()) {
			if decoder_info.width != metadata.width as usize || decoder_info.height != metadata.height as usize {
				return Err(JobError::MismatchedDimensions {
					width: decoder_info.width,
					height: decoder_info.height,
					expected_width: metadata.width as usize,
					expected_height: metadata.height as usize,
				});
			}

			if let Some(frame_count) = metadata.frame_count {
				if decoder_info.frame_count != frame_count as usize {
					return Err(JobError::MismatchedFrameCount {
						frame_count: decoder_info.frame_count,
						expected_frame_count: frame_count as usize,
					});
				}
			}

			if let Some(static_frame_index) = metadata.static_frame_index {
				if static_frame_index as usize >= decoder_info.frame_count {
					return Err(JobError::StaticFrameIndexOutOfBounds {
						idx: static_frame_index as usize,
						frame_count: decoder_info.frame_count,
					});
				}
			}
		}

		let mut frame_configs = vec![None; decoder_info.frame_count];

		if let Some(anim_config) = anim_config {
			match anim_config.frame_rate.as_ref() {
				Some(animation_config::FrameRate::FrameDurationsMs(durations)) => {
					if durations.values.len() != decoder_info.frame_count {
						return Err(JobError::MismatchedFrameCount {
							frame_count: decoder_info.frame_count,
							expected_frame_count: durations.values.len(),
						});
					}

					for (idx, duration) in durations.values.iter().enumerate() {
						frame_configs[idx] = Some(FrameConfig::DurationMs(*duration))
					}
				}
				Some(animation_config::FrameRate::FrameDurationMs(duration)) => {
					for config in frame_configs.iter_mut() {
						*config = Some(FrameConfig::DurationMs(*duration))
					}
				}
				_ => {}
			}

			for idx in anim_config.remove_frame_idxs.iter() {
				let idx = *idx as usize;
				if idx > decoder_info.frame_count {
					return Err(JobError::MismatchedFrameCount {
						frame_count: decoder_info.frame_count,
						expected_frame_count: idx + 1,
					});
				}

				frame_configs[idx] = Some(FrameConfig::Skip);
			}
		}

		let resizer = ImageResizer::new(&decoder_info, output)?;

		let loop_count = anim_config
			.and_then(|anim_config| anim_config.loop_count)
			.map(|loop_count| {
				if loop_count < 0 {
					LoopCount::Infinite
				} else {
					LoopCount::Finite(loop_count as usize)
				}
			})
			.unwrap_or(decoder_info.loop_count);

		let static_encoders = static_formats
			.into_iter()
			.map(|(f_idx, format_options)| {
				build_encoder_set(format_options, resizer.outputs(), loop_count).map(|encoders| (f_idx, encoders))
			})
			.collect::<Result<Vec<_>, JobError>>()?;

		let anim_encoders = if decoder_info.frame_count > 1 {
			anim_formats
				.into_iter()
				.map(|(f_idx, format_options)| {
					build_encoder_set(format_options, resizer.outputs(), loop_count).map(|encoders| (f_idx, encoders))
				})
				.collect::<Result<Vec<_>, JobError>>()?
		} else if !anim_formats.is_empty() && !output.skip_impossible_formats {
			return Err(JobError::ImpossibleOutput(anim_formats[0].1.format()));
		} else {
			Vec::new()
		};

		if static_encoders.is_empty() && anim_encoders.is_empty() {
			return Err(JobError::NoPossibleOutputs);
		}

		let static_frame_idx = task
			.input
			.as_ref()
			.and_then(|input| input.metadata.as_ref())
			.and_then(|metadata| metadata.static_frame_index)
			.unwrap_or_default() as usize;

		Ok(Self {
			decoder,
			decoder_info,
			frame_configs,
			resizer,
			static_encoders,
			anim_encoders,
			static_frame_idx,
			frame_idx: 0,
			duration_carried_ms: 0.0,
			input: InputImage {
				format: file_format,
				width: decoder_info.width,
				height: decoder_info.height,
				loop_count: decoder_info.loop_count,
				duration_ms: 0,
				frame_count: decoder_info.frame_count,
			},
			loop_count,
			frame_rate_factor: anim_config.and_then(|config| match config.frame_rate.as_ref()? {
				animation_config::FrameRate::FrameRateFactor(factor) => Some(*factor),
				_ => None,
			}),
		})
	}

	pub fn drive(&mut self) -> Result<bool, JobError> {
		let Some(mut frame) = self.decoder.decode()? else {
			return Ok(false);
		};

		let idx = self.frame_idx;
		self.frame_idx += 1;

		let variants = if idx == self.static_frame_idx {
			let variants = self.resizer.resize(frame)?;

			self.static_encoders.iter_mut().try_for_each(|(_, encoders)| {
				encoders
					.iter_mut()
					.zip(variants.iter())
					.try_for_each(|((_, encoder), frame)| encoder.add_frame(frame.as_ref()))
			})?;

			Some(variants)
		} else {
			None
		};

		// Convert from the decode timescale into ms.
		frame.duration_ts = (frame.duration_ts as f64 * 1000.0 / self.decoder_info.timescale as f64).round() as u64;

		if let Some(config) = self.frame_configs.get(idx).ok_or(JobError::Internal(""))? {
			match config {
				FrameConfig::Skip => {
					return Ok(true);
				}
				FrameConfig::DurationMs(duration) => {
					frame.duration_ts = *duration as u64;
				}
			}
		}

		if let Some(factor) = self.frame_rate_factor {
			let new_duration = (frame.duration_ts as f64 + self.duration_carried_ms) / factor;
			let rounded_duration = new_duration.round();
			self.duration_carried_ms = new_duration - rounded_duration;

			if rounded_duration == 0.0 {
				return Ok(true);
			}

			frame.duration_ts = rounded_duration as u64;
		}

		let variants = match variants {
			Some(variants) => variants,
			None => self.resizer.resize(frame)?,
		};

		self.anim_encoders.iter_mut().try_for_each(|(_, encoders)| {
			encoders
				.iter_mut()
				.zip(variants.iter())
				.try_for_each(|((_, encoder), frame)| encoder.add_frame(frame.as_ref()))
		})?;

		Ok(true)
	}

	pub fn finish(mut self) -> Result<JobOutput, JobError> {
		let output = self
			.static_encoders
			.into_iter()
			.chain(self.anim_encoders)
			.flat_map(|(f_idx, encoders)| {
				encoders.into_iter().map(move |(output, encoder)| {
					let info = encoder.info();
					Ok(OutputImage {
						format: info.format,
						format_name: info.name.clone(),
						format_idx: f_idx,
						resize_idx: output.index,
						scale: output.scale.map(|s| s as usize),
						width: info.width,
						height: info.height,
						frame_count: info.frame_count,
						duration_ms: info.duration,
						data: encoder.finish()?,
						loop_count: self.loop_count,
					})
				})
			})
			.collect::<Result<_, EncoderError>>()?;

		// Update the duration of the input.
		self.input.duration_ms = u64::try_from(self.decoder.duration_ms()).unwrap_or(0);

		Ok(JobOutput {
			input: self.input,
			output,
		})
	}
}
