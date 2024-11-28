use postgres_from_row::FromRow;
use ulid::Ulid;

use super::DatabaseTable;

#[derive(Debug, Clone, FromRow)]
pub struct RecordingThumbnail {
	/// The organization this recording thumbnail belongs to (primary key)
	pub organization_id: Ulid,
	/// The recording this thumbnail belongs to (primary key)
	pub recording_id: Ulid,
	/// The index of the thumbnail (primary key)
	pub idx: i32,

	/// The unique id for the thumbnail
	pub id: Ulid,

	/// The time the thumbnail was taken (relative to the start of the
	/// recording)
	pub start_time: f32,

	/// The size of the thumbnail in bytes
	pub size_bytes: i64,
}

impl DatabaseTable for RecordingThumbnail {
	const FRIENDLY_NAME: &'static str = "recording thumbnail";
	const NAME: &'static str = "recording_thumbnails";
}
