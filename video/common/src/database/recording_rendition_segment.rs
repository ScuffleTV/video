use postgres_from_row::FromRow;
use ulid::Ulid;

use super::{DatabaseTable, Rendition};

#[derive(Debug, Clone, FromRow)]
pub struct RecordingRenditionSegment {
	/// The organization this recording rendition segment belongs to (primary
	/// key)
	pub organization_id: Ulid,
	/// The recording this rendition segment belongs to (primary key)
	pub recording_id: Ulid,
	/// The rendition this segment belongs to (primary key)
	pub rendition: Rendition,
	/// The index of the segment (primary key)
	pub idx: i32,

	/// The unique id for the segment
	pub id: Ulid,

	/// The start time of the segment (relative to the start of the recording)
	pub start_time: f32,

	/// The end time of the segment (relative to the start of the recording)
	pub end_time: f32,

	/// The size of the segment in bytes
	pub size_bytes: i32,
}

impl DatabaseTable for RecordingRenditionSegment {
	const FRIENDLY_NAME: &'static str = "recording rendition segment";
	const NAME: &'static str = "recording_rendition_segments";
}
