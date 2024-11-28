use postgres_from_row::FromRow;
use ulid::Ulid;

use super::{DatabaseTable, Rendition};

#[derive(Debug, Clone, FromRow)]
pub struct RecordingRendition {
	/// The organization this recording rendition belongs to (primary key)
	pub organization_id: Ulid,
	/// A recording id (primary key)
	pub recording_id: Ulid,
	/// The rendition (primary key)
	pub rendition: Rendition,

	/// The config for the recording rendition pair
	/// This is a serialized protobuf, however it can be one of two types:
	/// - `pb::scuffle::video::v1::types::VideoConfig`
	/// - `pb::scuffle::video::v1::types::AudioConfig`
	/// Because of this, we store it as a `Vec<u8>` and deserialize it when
	/// needed.
	pub config: Vec<u8>,
}

impl DatabaseTable for RecordingRendition {
	const FRIENDLY_NAME: &'static str = "recording rendition";
	const NAME: &'static str = "recording_renditions";
}
