use std::collections::VecDeque;

use crate::transcoder::job::track::parser::TrackSample;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakType {
	Part,
	Segment,
}

#[derive(Debug)]
pub struct BreakpointState<'a> {
	samples: &'a VecDeque<TrackSample>,
	timescale: u32,
	idx: usize,
	break_points: Vec<(usize, BreakType)>,
	durations: Durations,
	potential_breaks: PotentialBreaks,
}

#[derive(Debug)]
struct Durations {
	part: u32,
	segment: u32,
	last_part: u32,
}

#[derive(Debug)]
struct PotentialBreaks {
	part: Option<(usize, u32, u32)>,
	segment: Option<(usize, f64)>,
}

impl<'a> BreakpointState<'a> {
	pub fn new(timescale: u32, segment_duration: u32, samples: &'a VecDeque<TrackSample>) -> Self {
		Self {
			samples,
			timescale,
			idx: 0,
			break_points: vec![],
			durations: Durations {
				part: 0,
				segment: segment_duration,
				last_part: 0,
			},
			potential_breaks: PotentialBreaks {
				part: None,
				segment: None,
			},
		}
	}

	pub fn into_breakpoints(self) -> Vec<(usize, BreakType)> {
		self.break_points
	}

	pub fn increment(&mut self) {
		self.idx += 1;
	}

	pub fn add_duration(&mut self) {
		let duration = self.current_sample_duration();

		self.durations.part += duration;
		self.durations.segment += duration;
	}

	pub fn process_segment_break(&mut self, target_segment_duration: f64, max_part_duration: f64) -> bool {
		if self.is_segment_break(target_segment_duration) && self.add_segment_break(max_part_duration) {
			true
		} else if let Some(idx) = self.check_potential_segment_break(max_part_duration) {
			self.idx = idx;
			self.force_segment_break(max_part_duration);
			true
		} else {
			false
		}
	}

	pub fn process_part_break(&mut self, target_part_duration: f64, max_part_duration: f64) -> bool {
		if self.is_part_break(target_part_duration) && self.add_part_break() {
			true
		} else if let Some((idx, part_duration, segment_duration)) = self.check_potential_part_break(max_part_duration) {
			self.idx = idx;
			self.durations.part = part_duration;
			self.durations.segment = segment_duration;
			self.force_part_break();
			true
		} else {
			false
		}
	}

	fn segment_time(&self) -> f64 {
		(self.durations.segment - self.current_sample_duration()) as f64 / self.timescale as f64
	}

	fn part_time(&self) -> f64 {
		self.durations.part as f64 / self.timescale as f64
	}

	fn is_perfect_segment_break(&self) -> bool {
		(self.segment_time() * 1000.0).fract() == 0.0
	}

	fn is_perfect_part_break(&self) -> bool {
		(self.part_time() * 1000.0).fract() == 0.0
	}

	fn is_part_break(&self, target_part_duration: f64) -> bool {
		self.potential_breaks.segment.is_none() && self.part_time() >= target_part_duration
	}

	fn is_segment_break(&self, target_segment_duration: f64) -> bool {
		self.current_sample().map(|s| s.keyframe).unwrap_or_default() && self.segment_time() >= target_segment_duration
	}

	fn merge_last_breakpoint(&mut self, max_part_duration: f64) {
		if let Some((_, breaktype)) = self.break_points.last() {
			if *breaktype == BreakType::Part
				&& (self.durations.last_part + self.durations.part - self.current_sample_duration()) as f64
					/ self.timescale as f64
					<= max_part_duration
			{
				// If the last break point was a part break, and the last part duration + this
				// part duration is less than the max part duration, we remove the last break
				// point. This is because we want to merge the last part with this part.
				self.break_points.pop();
			}
		}
	}

	fn check_potential_segment_break(&mut self, max_part_duration: f64) -> Option<usize> {
		if let Some((idx, t)) = self.potential_breaks.segment {
			if t + max_part_duration < self.segment_time() {
				return Some(idx);
			}
		}

		None
	}

	fn check_potential_part_break(&self, max_part_duration: f64) -> Option<(usize, u32, u32)> {
		if self.part_time() >= max_part_duration {
			self.potential_breaks.part
		} else {
			None
		}
	}

	fn add_segment_break(&mut self, max_part_duration: f64) -> bool {
		if self.is_perfect_segment_break() {
			self.force_segment_break(max_part_duration);
			true
		} else if self.potential_breaks.segment.is_none() {
			self.potential_breaks.segment = Some((self.idx, self.segment_time()));
			false
		} else {
			false
		}
	}

	fn add_part_break(&mut self) -> bool {
		if self.is_perfect_part_break() {
			self.force_part_break();
			true
		} else if self.potential_breaks.part.is_none() {
			self.potential_breaks.part = Some((self.idx, self.durations.part, self.durations.segment));
			false
		} else {
			false
		}
	}

	fn force_segment_break(&mut self, max_part_duration: f64) {
		self.merge_last_breakpoint(max_part_duration);

		self.break_points.push((self.idx, BreakType::Segment));
		self.durations.segment = self.current_sample_duration();
		self.durations.part = self.current_sample_duration();
		self.potential_breaks.part = None;
		self.potential_breaks.segment = None;
	}

	fn force_part_break(&mut self) {
		self.break_points.push((self.idx + 1, BreakType::Part));
		self.durations.last_part = self.durations.part;
		self.durations.part = 0;
		self.potential_breaks.part = None;
		self.potential_breaks.segment = None;
	}

	pub fn current_sample(&self) -> Option<&TrackSample> {
		self.samples.get(self.idx)
	}

	fn current_sample_duration(&self) -> u32 {
		self.current_sample().map(|s| s.duration).unwrap_or_default()
	}
}
