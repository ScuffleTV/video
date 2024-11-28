use ffmpeg_sys_next::*;

use crate::error::FfmpegError;
use crate::smart_object::SmartPtr;
use crate::utils::check_i64;

pub struct Frame(SmartPtr<AVFrame>);

impl Clone for Frame {
	fn clone(&self) -> Self {
		unsafe { Self::wrap(av_frame_clone(self.0.as_ptr())).expect("failed to clone frame") }
	}
}

/// Safety: `Frame` is safe to send between threads.
unsafe impl Send for Frame {}

/// Safety: `Frame` is safe to share between threads.
unsafe impl Sync for Frame {}

#[derive(Clone)]
pub struct VideoFrame(pub Frame);

#[derive(Clone)]
pub struct AudioFrame(pub Frame);

impl Frame {
	pub fn new() -> Result<Self, FfmpegError> {
		// Safety: the pointer returned from av_frame_alloc is valid
		unsafe { Self::wrap(av_frame_alloc()) }
	}

	/// Safety: `ptr` must be a valid pointer to an `AVFrame`.
	unsafe fn wrap(ptr: *mut AVFrame) -> Result<Self, FfmpegError> {
		Ok(Self(
			// The caller guarantees that `ptr` is valid.
			SmartPtr::wrap_non_null(ptr, |ptr| av_frame_free(ptr)).ok_or(FfmpegError::Alloc)?,
		))
	}

	pub fn as_ptr(&self) -> *const AVFrame {
		self.0.as_ptr()
	}

	pub fn as_mut_ptr(&mut self) -> *mut AVFrame {
		self.0.as_mut_ptr()
	}

	pub fn video(self) -> VideoFrame {
		VideoFrame(self)
	}

	pub fn audio(self) -> AudioFrame {
		AudioFrame(self)
	}

	pub fn pts(&self) -> Option<i64> {
		check_i64(self.0.as_deref_except().pts)
	}

	pub fn set_pts(&mut self, pts: Option<i64>) {
		self.0.as_deref_mut_except().pts = pts.unwrap_or(AV_NOPTS_VALUE);
		self.0.as_deref_mut_except().best_effort_timestamp = pts.unwrap_or(AV_NOPTS_VALUE);
	}

	pub fn duration(&self) -> Option<i64> {
		check_i64(self.0.as_deref_except().duration).or_else(|| check_i64(self.0.as_deref_except().pkt_duration))
	}

	pub fn set_duration(&mut self, duration: Option<i64>) {
		self.0.as_deref_mut_except().duration = duration.unwrap_or(AV_NOPTS_VALUE);
		self.0.as_deref_mut_except().pkt_duration = duration.unwrap_or(AV_NOPTS_VALUE);
	}

	pub fn best_effort_timestamp(&self) -> Option<i64> {
		check_i64(self.0.as_deref_except().best_effort_timestamp)
	}

	pub fn dts(&self) -> Option<i64> {
		check_i64(self.0.as_deref_except().pkt_dts)
	}

	pub fn set_dts(&mut self, dts: Option<i64>) {
		self.0.as_deref_mut_except().pkt_dts = dts.unwrap_or(AV_NOPTS_VALUE);
	}

	pub fn time_base(&self) -> AVRational {
		self.0.as_deref_except().time_base
	}

	pub fn set_time_base(&mut self, time_base: AVRational) {
		self.0.as_deref_mut_except().time_base = time_base;
	}

	pub fn format(&self) -> i32 {
		self.0.as_deref_except().format
	}

	pub fn set_format(&mut self, format: i32) {
		self.0.as_deref_mut_except().format = format;
	}

	pub fn is_audio(&self) -> bool {
		self.0.as_deref_except().channel_layout != 0
	}

	pub fn is_video(&self) -> bool {
		self.0.as_deref_except().width != 0
	}

	pub fn linesize(&self, index: usize) -> Option<i32> {
		self.0.as_deref_except().linesize.get(index).copied()
	}
}

impl std::fmt::Debug for Frame {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Frame")
			.field("pts", &self.pts())
			.field("dts", &self.dts())
			.field("duration", &self.duration())
			.field("best_effort_timestamp", &self.best_effort_timestamp())
			.field("time_base", &self.time_base())
			.field("format", &self.format())
			.field("is_audio", &self.is_audio())
			.field("is_video", &self.is_video())
			.finish()
	}
}

impl VideoFrame {
	pub fn width(&self) -> usize {
		self.0 .0.as_deref_except().width as usize
	}

	pub fn height(&self) -> usize {
		self.0 .0.as_deref_except().height as usize
	}

	pub fn sample_aspect_ratio(&self) -> AVRational {
		self.0 .0.as_deref_except().sample_aspect_ratio
	}

	pub fn set_sample_aspect_ratio(&mut self, sample_aspect_ratio: AVRational) {
		self.0 .0.as_deref_mut_except().sample_aspect_ratio = sample_aspect_ratio;
	}

	pub fn set_width(&mut self, width: usize) {
		self.0 .0.as_deref_mut_except().width = width as i32;
	}

	pub fn set_height(&mut self, height: usize) {
		self.0 .0.as_deref_mut_except().height = height as i32;
	}

	pub fn is_keyframe(&self) -> bool {
		self.0 .0.as_deref_except().key_frame != 0
	}

	pub fn pict_type(&self) -> AVPictureType {
		self.0 .0.as_deref_except().pict_type
	}

	pub fn set_pict_type(&mut self, pict_type: AVPictureType) {
		self.0 .0.as_deref_mut_except().pict_type = pict_type;
	}

	pub fn data(&self, index: usize) -> Option<&[u8]> {
		unsafe {
			self.0
				 .0
				.as_deref_except()
				.data
				.get(index)
				.map(|ptr| std::slice::from_raw_parts(*ptr, self.linesize(index).unwrap() as usize * self.height()))
		}
	}
}

impl std::fmt::Debug for VideoFrame {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("VideoFrame")
			.field("width", &self.width())
			.field("height", &self.height())
			.field("sample_aspect_ratio", &self.sample_aspect_ratio())
			.field("pts", &self.pts())
			.field("dts", &self.dts())
			.field("duration", &self.duration())
			.field("best_effort_timestamp", &self.best_effort_timestamp())
			.field("time_base", &self.time_base())
			.field("format", &self.format())
			.field("is_audio", &self.is_audio())
			.field("is_video", &self.is_video())
			.field("is_keyframe", &self.is_keyframe())
			.finish()
	}
}

impl std::ops::Deref for VideoFrame {
	type Target = Frame;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl std::ops::DerefMut for VideoFrame {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}

impl AudioFrame {
	pub fn nb_samples(&self) -> i32 {
		self.0 .0.as_deref_except().nb_samples
	}

	pub fn set_nb_samples(&mut self, nb_samples: usize) {
		self.0 .0.as_deref_mut_except().nb_samples = nb_samples as i32;
	}

	pub fn sample_rate(&self) -> i32 {
		self.0 .0.as_deref_except().sample_rate
	}

	pub fn set_sample_rate(&mut self, sample_rate: usize) {
		self.0 .0.as_deref_mut_except().sample_rate = sample_rate as i32;
	}

	pub fn channel_layout(&self) -> u64 {
		self.0 .0.as_deref_except().channel_layout
	}

	pub fn set_channel_layout(&mut self, channel_layout: u64) {
		self.0 .0.as_deref_mut_except().channel_layout = channel_layout;
	}
}

impl std::fmt::Debug for AudioFrame {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("AudioFrame")
			.field("nb_samples", &self.nb_samples())
			.field("sample_rate", &self.sample_rate())
			.field("channel_layout", &self.channel_layout())
			.field("pts", &self.pts())
			.field("dts", &self.dts())
			.field("duration", &self.duration())
			.field("best_effort_timestamp", &self.best_effort_timestamp())
			.field("time_base", &self.time_base())
			.field("format", &self.format())
			.field("is_audio", &self.is_audio())
			.field("is_video", &self.is_video())
			.finish()
	}
}

impl std::ops::Deref for AudioFrame {
	type Target = Frame;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

impl std::ops::DerefMut for AudioFrame {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.0
	}
}
