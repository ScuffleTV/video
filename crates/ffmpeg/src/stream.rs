use ffmpeg_sys_next::*;

use crate::consts::{Const, Mut};
use crate::dict::Dictionary;
use crate::utils::check_i64;

pub struct Streams<'a> {
	input: &'a AVFormatContext,
}

/// Safety: `Streams` is safe to send between threads.
unsafe impl Send for Streams<'_> {}

impl<'a> Streams<'a> {
	pub(crate) fn new(input: &'a AVFormatContext) -> Self {
		Self { input }
	}

	pub fn best(&self, media_type: AVMediaType) -> Option<Const<'a, Stream<'a>>> {
		// Safety: av_find_best_stream is safe to call, 'input' is a valid pointer
		// We upcast the pointer to a mutable pointer because the function signature
		// requires it, but it does not mutate the pointer.
		let stream =
			unsafe { av_find_best_stream(self.input as *const _ as *mut _, media_type, -1, -1, std::ptr::null_mut(), 0) };
		if stream < 0 {
			return None;
		}

		// Safety: if we get back an index, it's valid
		let stream = unsafe { &mut *(*self.input.streams.add(stream as usize)) };

		Some(Const::new(Stream::new(stream, self.input)))
	}

	pub fn best_mut(&mut self, media_type: AVMediaType) -> Option<Stream<'a>> {
		self.best(media_type).map(|s| s.0)
	}
}

impl<'a> IntoIterator for Streams<'a> {
	type IntoIter = StreamIter<'a>;
	type Item = Const<'a, Stream<'a>>;

	fn into_iter(self) -> Self::IntoIter {
		StreamIter {
			input: self.input,
			index: 0,
		}
	}
}

impl<'a> Streams<'a> {
	pub fn iter(&'a self) -> StreamIter<'a> {
		StreamIter {
			input: self.input,
			index: 0,
		}
	}

	pub fn len(&self) -> usize {
		self.input.nb_streams as usize
	}

	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	pub fn get(&mut self, index: usize) -> Option<Stream<'_>> {
		if index >= self.len() {
			return None;
		}

		let stream = unsafe { &mut *(*self.input.streams.add(index)) };
		Some(Stream::new(stream, self.input))
	}
}

pub struct StreamIter<'a> {
	input: &'a AVFormatContext,
	index: usize,
}

impl<'a> Iterator for StreamIter<'a> {
	type Item = Const<'a, Stream<'a>>;

	fn next(&mut self) -> Option<Self::Item> {
		if self.index >= self.input.nb_streams as usize {
			return None;
		}

		let stream = unsafe { &mut *(*self.input.streams.add(self.index)) };
		self.index += 1;

		Some(Const::new(Stream::new(stream, self.input)))
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		let remaining = self.input.nb_streams as usize - self.index;
		(remaining, Some(remaining))
	}
}

impl std::iter::ExactSizeIterator for StreamIter<'_> {}

pub struct Stream<'a>(&'a mut AVStream, &'a AVFormatContext);

impl<'a> Stream<'a> {
	pub(crate) fn new(stream: &'a mut AVStream, input: &'a AVFormatContext) -> Self {
		Self(stream, input)
	}

	pub fn as_ptr(&self) -> *const AVStream {
		self.0
	}

	pub fn as_mut_ptr(&mut self) -> *mut AVStream {
		self.0
	}
}

impl<'a> Stream<'a> {
	pub fn index(&self) -> i32 {
		self.0.index
	}

	pub fn id(&self) -> i32 {
		self.0.id
	}

	pub fn codec_parameters(&self) -> Option<&'a AVCodecParameters> {
		// Safety: the pointer is valid
		unsafe { self.0.codecpar.as_ref() }
	}

	pub fn time_base(&self) -> AVRational {
		self.0.time_base
	}

	pub fn set_time_base(&mut self, time_base: AVRational) {
		self.0.time_base = time_base;
	}

	pub fn start_time(&self) -> Option<i64> {
		check_i64(self.0.start_time)
	}

	pub fn set_start_time(&mut self, start_time: Option<i64>) {
		self.0.start_time = start_time.unwrap_or(AV_NOPTS_VALUE)
	}

	pub fn duration(&self) -> Option<i64> {
		check_i64(self.0.duration)
	}

	pub fn set_duration(&mut self, duration: Option<i64>) {
		self.0.duration = duration.unwrap_or(AV_NOPTS_VALUE)
	}

	pub fn nb_frames(&self) -> Option<i64> {
		check_i64(self.0.nb_frames)
	}

	pub fn set_nb_frames(&mut self, nb_frames: i64) {
		self.0.nb_frames = nb_frames;
	}

	pub fn disposition(&self) -> i32 {
		self.0.disposition
	}

	pub fn set_disposition(&mut self, disposition: i32) {
		self.0.disposition = disposition;
	}

	pub fn discard(&self) -> AVDiscard {
		self.0.discard
	}

	pub fn set_discard(&mut self, discard: AVDiscard) {
		self.0.discard = discard;
	}

	pub fn sample_aspect_ratio(&self) -> AVRational {
		self.0.sample_aspect_ratio
	}

	pub fn set_sample_aspect_ratio(&mut self, sample_aspect_ratio: AVRational) {
		self.0.sample_aspect_ratio = sample_aspect_ratio;
	}

	pub fn metadata(&self) -> Const<'_, Dictionary> {
		// Safety: the pointer metadata pointer does not live longer than this object,
		// see `Const::new`
		Const::new(unsafe { Dictionary::from_ptr_ref(self.0.metadata) })
	}

	pub fn metadata_mut(&mut self) -> Mut<'_, Dictionary> {
		// Safety: the pointer metadata pointer does not live longer than this object,
		// see `Mut::new`
		Mut::new(unsafe { Dictionary::from_ptr_ref(self.0.metadata) })
	}

	pub fn avg_frame_rate(&self) -> AVRational {
		self.0.avg_frame_rate
	}

	pub fn r_frame_rate(&self) -> AVRational {
		self.0.r_frame_rate
	}

	pub fn format_context(&self) -> &'a AVFormatContext {
		self.1
	}
}

impl std::fmt::Debug for Stream<'_> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Stream")
			.field("index", &self.index())
			.field("id", &self.id())
			.field("time_base", &self.time_base())
			.field("start_time", &self.start_time())
			.field("duration", &self.duration())
			.field("nb_frames", &self.nb_frames())
			.field("disposition", &self.disposition())
			.field("discard", &self.discard())
			.field("sample_aspect_ratio", &self.sample_aspect_ratio())
			.field("metadata", &self.metadata())
			.field("avg_frame_rate", &self.avg_frame_rate())
			.field("r_frame_rate", &self.r_frame_rate())
			.finish()
	}
}
