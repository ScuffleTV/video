use video_player_types::RenditionPlaylistSegment;

#[derive(Debug, Clone, Copy)]
pub struct TimeRegion {
	pub start: f64,
	pub end: f64,
}

#[derive(Debug, Clone, Default)]
pub struct TimeRegions {
	regions: Vec<TimeRegion>,
}

impl TimeRegions {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn clear(&mut self) {
		self.regions.clear();
	}

	pub fn add(&mut self, start: f64, end: f64) {
		self.regions.push(TimeRegion { start, end });

		// Sort by start
		self.regions.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap());

		// Merge overlapping regions
		let mut i = 0;
		while i < self.regions.len() - 1 {
			let a = &self.regions[i];
			let b = &self.regions[i + 1];

			if a.end >= b.start {
				self.regions[i].end = b.end;
				self.regions.remove(i + 1);
			} else {
				i += 1;
			}
		}
	}

	pub fn remove(&mut self, start: f64, end: f64) {
		let mut idx = 0;
		while idx < self.regions.len() {
			if self.regions[idx].end <= start {
				// Current region is entirely before the interval.
				idx += 1;
				continue;
			} else if self.regions[idx].start >= end {
				// Current region is entirely after the interval.
				break;
			} else if self.regions[idx].start < start && self.regions[idx].end > start && self.regions[idx].end <= end {
				// Only the end of the region is overlapping.
				self.regions[idx].end = start;
				idx += 1;
			} else if self.regions[idx].start >= start && self.regions[idx].start < end && self.regions[idx].end > end {
				// Only the start of the region is overlapping.
				self.regions[idx].start = end;
				idx += 1;
			} else if self.regions[idx].start < start && self.regions[idx].end > end {
				// The interval splits the region into two.
				let old_end = self.regions[idx].end;
				self.regions[idx].end = start;
				self.regions.insert(
					idx + 1,
					TimeRegion {
						start: end,
						end: old_end,
					},
				);
				idx += 2;
			} else {
				// The region is entirely encompassed by the interval.
				self.regions.remove(idx);
			}
		}
	}

	pub fn get(&self, time: f64) -> Option<&TimeRegion> {
		self.regions.iter().find(|region| region.start <= time && region.end >= time)
	}
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct SegmentRegions {
	regions: Vec<SegmentRegion>,
}

#[derive(Debug, Clone, Copy)]
pub enum SegmentRangeResult<'a> {
	Active,
	Discontinuity(Option<&'a SegmentRegion>, Option<&'a SegmentRegion>),
	Range(&'a SegmentRegion),
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SegmentRegion {
	pub idx: u32,
	pub start: f64,
	pub end: f64,
	pub dvr_tag: String,
	pub active: bool,
}

impl SegmentRegions {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn clear(&mut self) {
		self.regions.clear();
	}

	pub fn is_empty(&self) -> bool {
		self.regions.is_empty()
	}

	pub fn active_range(&mut self, start: f64) {
		// We need to make sure that all segments before start are marjed as inactive.
		// We also need to make sure that all segments after start are marked as active.
		for region in self.regions.iter_mut().rev() {
			let active = region.start >= start;
			region.active = active;
		}
	}

	pub fn add(&mut self, segment: &RenditionPlaylistSegment) {
		if segment.start_time.is_none() || segment.dvr_tag.is_none() {
			return;
		}

		let segment = SegmentRegion {
			dvr_tag: segment.dvr_tag.clone().unwrap(),
			end: segment.end_time.unwrap(),
			idx: segment.idx,
			start: segment.start_time.unwrap(),
			active: segment.id.is_some(),
		};

		// Binary search for old segment
		match self.regions.binary_search_by(|probe| probe.idx.cmp(&segment.idx)) {
			Ok(idx) => {
				// Replace old segment
				self.regions[idx] = segment;
			}
			Err(idx) => {
				// Insert new segment
				self.regions.insert(idx, segment);
			}
		}
	}

	pub fn get(&self, time: f64) -> SegmentRangeResult<'_> {
		if time < 0.0 {
			return SegmentRangeResult::Active;
		}

		// Binary search for the first time that start time is greater than time
		match self.regions.binary_search_by(|probe| probe.start.partial_cmp(&time).unwrap()) {
			Ok(idx) => {
				// Found exact match
				let s = &self.regions[idx];
				if s.active {
					SegmentRangeResult::Active
				} else {
					SegmentRangeResult::Range(s)
				}
			}
			Err(idx) => {
				// Found segment that starts after time
				if idx > 0 {
					let s = &self.regions[idx - 1];
					if s.active {
						SegmentRangeResult::Active
					} else if s.end < time {
						SegmentRangeResult::Discontinuity(Some(s), self.regions.get(idx))
					} else {
						SegmentRangeResult::Range(s)
					}
				} else if let Some(first) = self.regions.first() {
					SegmentRangeResult::Discontinuity(None, Some(first))
				} else {
					SegmentRangeResult::Active
				}
			}
		}
	}
}
