use pb::scuffle::video::internal::live_rendition_manifest::RenditionInfo;

#[derive(Debug, Clone, Copy)]
pub enum BlockStyle {
	Hls { msn: u32, part: u32 },
	ScufflePart(u32),
	ScuffleIPart(u32),
}

impl BlockStyle {
	pub fn is_blocked(&self, info: &RenditionInfo) -> bool {
		let segment_idx = info.next_segment_idx.saturating_sub(1);
		let part_idx = info.next_part_idx.saturating_sub(1);
		let segment_part_idx = info.next_segment_part_idx.saturating_sub(1);

		match *self {
			BlockStyle::Hls { msn, part } => segment_idx < msn || (segment_idx == msn && segment_part_idx < part),
			BlockStyle::ScufflePart(scuffle_part) => part_idx < scuffle_part,
			BlockStyle::ScuffleIPart(scuffle_ipart) => info.last_independent_part_idx <= scuffle_ipart,
		}
	}
}
