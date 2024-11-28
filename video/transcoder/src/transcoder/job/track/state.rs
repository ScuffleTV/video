use std::collections::VecDeque;

use bytes::Bytes;
use bytesio::bytes_writer::BytesWriter;
use mp4::types::ftyp::{FourCC, Ftyp};
use mp4::types::mdat::Mdat;
use mp4::types::mfhd::Mfhd;
use mp4::types::moof::Moof;
use mp4::types::moov::Moov;
use mp4::types::mvex::Mvex;
use mp4::types::mvhd::Mvhd;
use mp4::types::tfdt::Tfdt;
use mp4::types::tfhd::Tfhd;
use mp4::types::traf::Traf;
use mp4::types::trex::Trex;
use mp4::types::trun::Trun;
use mp4::BoxType;
use pb::ext::UlidExt;
use pb::scuffle::video::internal::LiveRenditionManifest;
use ulid::Ulid;

use crate::transcoder::job::breakpoint::{BreakType, BreakpointState};
use crate::transcoder::job::track::parser::TrackSample;

#[derive(Default, Clone)]
pub struct Part {
	pub data: Bytes,
	pub duration: u32,
	pub idx: u32,
	pub start_ts: u64,
	pub independent: bool,
}

impl std::fmt::Debug for Part {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Part")
			.field("duration", &self.duration)
			.field("idx", &self.idx)
			.field("independent", &self.independent)
			.finish()
	}
}

#[derive(Default, Clone, Debug)]
pub struct Segment {
	pub parts: Vec<Part>,
	pub idx: u32,
	pub id: Ulid,
}

impl Segment {
	pub fn part(&self, idx: u32) -> Option<&Part> {
		self.parts.iter().find(|p| p.idx == idx)
	}

	pub fn duration(&self) -> u32 {
		self.parts.iter().map(|p| p.duration).sum()
	}
}

#[derive(Default, Clone)]
pub struct TrackState {
	samples: VecDeque<TrackSample>,

	timescale: u32,

	segments: VecDeque<Segment>,

	init_segment: Option<Bytes>,

	total_duration: u64,

	next_part_idx: u32,
	next_segment_idx: u32,
	next_segment_part_idx: u32,
	last_independent_part_idx: u32,

	complete: bool,
}

impl std::fmt::Debug for TrackState {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("TrackState")
			.field("timescale", &self.timescale)
			.field("segments", &self.segments)
			.field("total_duration", &self.total_duration)
			.field("next_part_idx", &self.next_part_idx)
			.field("next_segment_idx", &self.next_segment_idx)
			.field("next_segment_part_idx", &self.next_segment_part_idx)
			.field("last_independent_part_idx", &self.last_independent_part_idx)
			.field("complete", &self.complete)
			.finish()
	}
}

impl TrackState {
	pub fn timescale(&self) -> u32 {
		self.timescale
	}

	pub fn total_duration(&self) -> u64 {
		self.total_duration
	}

	pub fn segments(&self) -> impl Iterator<Item = &Segment> {
		self.segments.iter()
	}

	pub fn next_part_idx(&self) -> u32 {
		self.next_part_idx
	}

	pub fn next_segment_idx(&self) -> u32 {
		self.next_segment_idx
	}

	pub fn next_segment_part_idx(&self) -> u32 {
		self.next_segment_part_idx
	}

	pub fn last_independent_part_idx(&self) -> u32 {
		self.last_independent_part_idx
	}

	pub fn apply_manifest(&mut self, manifest: &LiveRenditionManifest) {
		let Some(info) = manifest.info.as_ref() else {
			return;
		};

		self.next_part_idx = info.next_part_idx;
		self.next_segment_idx = info.next_segment_idx;
		self.total_duration = manifest.total_duration;
		self.timescale = manifest.timescale;
		self.next_segment_part_idx = info.next_segment_part_idx;
		self.last_independent_part_idx = info.last_independent_part_idx;

		let mut segments = manifest
			.segments
			.iter()
			.map(|s| Segment {
				idx: s.idx,
				parts: s
					.parts
					.iter()
					.map(|p| Part {
						data: Bytes::new(),
						duration: p.duration,
						idx: p.idx,
						independent: p.independent,
						start_ts: 0,
					})
					.collect(),
				id: s.id.into_ulid(),
			})
			.collect::<Vec<_>>();

		segments.sort_unstable_by_key(|s| s.idx);

		self.segments = segments.into();

		self.segments.push_back(Segment {
			idx: self.next_segment_idx,
			parts: vec![],
			id: Ulid::new(),
		});

		self.next_segment_idx += 1;
	}

	pub fn complete(&self) -> bool {
		self.complete
	}

	pub fn segment(&self, idx: u32) -> Option<&Segment> {
		let segment = self.segments.front()?;
		if idx >= segment.idx {
			self.segments.get((idx - segment.idx) as usize)
		} else {
			None
		}
	}

	pub fn retain_segments(&mut self, count: usize) -> Vec<Segment> {
		(0..self.segments.len().saturating_sub(count))
			.filter_map(|_| self.segments.pop_front())
			.collect()
	}

	pub fn last_segment_duration(&self) -> u32 {
		self.segments
			.back()
			.map(|segment| segment.parts.iter().map(|p| p.duration).sum::<u32>())
			.unwrap_or_default()
	}

	pub fn set_moov(&mut self, mut moov: Moov) {
		let mut trak = moov.traks.remove(0);

		trak.edts = None;
		trak.tkhd.track_id = 1;

		self.timescale = trak.mdia.mdhd.timescale;

		let ftyp = Ftyp::new(FourCC::Iso5, 512, vec![FourCC::Iso5, FourCC::Iso6, FourCC::Mp41]);
		let moov = Moov::new(
			Mvhd::new(0, 0, 1000, 0, 2),
			vec![trak],
			Some(Mvex::new(vec![Trex::new(1)], None)),
		);

		let mut writer = BytesWriter::default();
		ftyp.mux(&mut writer).unwrap();
		moov.mux(&mut writer).unwrap();

		self.init_segment = Some(writer.dispose());
	}

	fn compute_break_points(
		&self,
		target_part_duration: f64,
		max_part_duration: f64,
		target_segment_duration: f64,
	) -> Vec<(usize, BreakType)> {
		let mut state = BreakpointState::new(self.timescale, self.last_segment_duration(), &self.samples);

		while state.current_sample().is_some() {
			state.add_duration();

			if !state.process_segment_break(target_segment_duration, max_part_duration) {
				state.process_part_break(target_part_duration, max_part_duration);
			}

			state.increment();
		}

		state.into_breakpoints()
	}

	pub fn finish(&mut self) -> Option<(u32, u32)> {
		self.complete = true;

		if self.samples.is_empty() {
			return None;
		}

		let samples = self.samples.drain(..).collect();
		let part = self.make_part(samples);
		let part_idx = part.idx;
		let segment_idx = if let Some(segment) = self.segments.back_mut() {
			segment.parts.push(part);
			segment.idx
		} else {
			self.segments.push_back(Segment {
				parts: vec![part],
				idx: self.next_segment_idx,
				id: Ulid::new(),
			});
			self.next_segment_idx += 1;
			self.next_segment_idx - 1
		};

		self.next_segment_part_idx = 0;

		Some((segment_idx, part_idx))
	}

	fn make_part(&mut self, samples: Vec<TrackSample>) -> Part {
		let contains_keyframe = samples.iter().any(|sample| sample.keyframe);
		let duration = samples.iter().map(|sample| sample.duration).sum::<u32>();

		let mut moof = Moof::new(
			Mfhd::new(self.next_part_idx),
			vec![{
				let mut traf = Traf::new(
					Tfhd::new(1, None, None, None, None, None),
					Some(Trun::new(samples.iter().map(|s| s.sample.clone()).collect(), None)),
					Some(Tfdt::new(self.total_duration)),
				);

				traf.optimize();

				traf
			}],
		);

		let moof_size = moof.size();
		moof.traf.get_mut(0).unwrap().trun.as_mut().unwrap().data_offset = Some(moof_size as i32 + 8);

		let mdat = Mdat::new(samples.into_iter().map(|s| s.data).collect::<Vec<_>>());

		let mut writer = BytesWriter::default();
		moof.mux(&mut writer).unwrap();
		mdat.mux(&mut writer).unwrap();

		let part = Part {
			data: writer.dispose(),
			duration,
			idx: self.next_part_idx,
			independent: contains_keyframe,
			start_ts: self.total_duration,
		};

		if part.independent {
			self.last_independent_part_idx = part.idx;
		}

		self.next_part_idx += 1;
		self.next_segment_part_idx += 1;
		self.total_duration += duration as u64;

		part
	}

	pub fn append_samples(&mut self, samples: Vec<TrackSample>) {
		self.samples.extend(samples);
	}

	pub fn init_segment(&self) -> Option<&Bytes> {
		self.init_segment.as_ref()
	}

	pub fn split_samples(
		&mut self,
		target_part_duration: f64,
		max_part_duration: f64,
		target_segment_duration: f64,
	) -> Vec<(u32, Vec<u32>)> {
		let break_points = self.compute_break_points(target_part_duration, max_part_duration, target_segment_duration);

		let segments = self.split_into_segments(break_points);

		self.initialize_segments_if_empty();

		self.convert_to_parts_and_extend_segments(segments)
	}

	fn split_into_segments(&mut self, break_points: Vec<(usize, BreakType)>) -> Vec<Vec<Vec<TrackSample>>> {
		let mut segments = vec![];
		let mut current_segment = vec![];
		let mut previous_break_idx = 0;

		for (break_idx, break_type) in break_points {
			let part = self.samples.drain(..break_idx - previous_break_idx).collect::<Vec<_>>();
			previous_break_idx = break_idx;

			match break_type {
				BreakType::Part => {
					debug_assert!(!part.is_empty());
					if current_segment.is_empty() && !segments.is_empty() {
						debug_assert!(part[0].keyframe);
					}
					current_segment.push(part);
				}
				BreakType::Segment => {
					if !part.is_empty() {
						current_segment.push(part);
					}
					segments.push(current_segment);
					current_segment = vec![];
				}
			}
		}

		if !current_segment.is_empty() {
			segments.push(current_segment);
		}

		segments
	}

	fn initialize_segments_if_empty(&mut self) {
		if self.segments.is_empty() {
			self.segments.push_back(Segment {
				parts: vec![],
				idx: self.next_segment_idx,
				id: Ulid::new(),
			});
			self.next_segment_idx += 1;
			self.next_segment_part_idx = 0;
		}
	}

	fn convert_to_parts_and_extend_segments(&mut self, segments: Vec<Vec<Vec<TrackSample>>>) -> Vec<(u32, Vec<u32>)> {
		let segment_count = segments.len();

		segments
			.into_iter()
			.enumerate()
			.map(|(idx, parts)| {
				let parts = parts.into_iter().map(|samples| self.make_part(samples)).collect::<Vec<_>>();
				let current_segment = self.segments.back_mut().unwrap();

				let part_ids = parts.iter().map(|p| p.idx).collect::<Vec<_>>();

				current_segment.parts.extend(parts);

				let segment_idx = current_segment.idx;

				if idx != segment_count - 1 {
					self.segments.push_back(Segment {
						parts: vec![],
						idx: self.next_segment_idx,
						id: Ulid::new(),
					});
					self.next_segment_idx += 1;
					self.next_segment_part_idx = 0;
				}

				(segment_idx, part_ids)
			})
			.collect()
	}
}
