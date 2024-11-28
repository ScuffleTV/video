use std::collections::HashMap;

use pb::scuffle::video::v1::types::{AudioConfig, RecordingConfig, TranscodingConfig, VideoConfig};
use postgres_from_row::FromRow;
use scuffle_utils::database::{json, protobuf_opt, protobuf_vec_opt};
use ulid::Ulid;

use super::{DatabaseTable, RoomStatus, Visibility};

#[derive(Debug, Clone, Default, FromRow)]
pub struct Room {
	/// The organization this room belongs to (primary key)
	pub organization_id: Ulid,
	/// A unique id for the room (primary key)
	pub id: Ulid,

	/// The transcoding config this room uses
	pub transcoding_config_id: Option<Ulid>,

	/// The recording config this room uses
	pub recording_config_id: Option<Ulid>,

	/// The visibility of the room
	pub visibility: Visibility,

	/// The stream key for the room
	pub stream_key: String,

	/// The date and time the room was last updated
	pub updated_at: chrono::DateTime<chrono::Utc>,

	/// The date and time the room was last live
	pub last_live_at: Option<chrono::DateTime<chrono::Utc>>,

	/// The date and time the room was last disconnected
	pub last_disconnected_at: Option<chrono::DateTime<chrono::Utc>>,

	/// The status of the room
	pub status: RoomStatus,

	/// The video input config for the active ingest connection
	#[from_row(from_fn = "protobuf_opt")]
	pub video_input: Option<VideoConfig>,

	/// The audio input config for the active ingest connection
	#[from_row(from_fn = "protobuf_opt")]
	pub audio_input: Option<AudioConfig>,

	/// The active ingest connection id
	pub active_ingest_connection_id: Option<Ulid>,

	/// The active recording config
	#[from_row(from_fn = "protobuf_opt")]
	pub active_recording_config: Option<RecordingConfig>,

	/// The active transcoding config
	#[from_row(from_fn = "protobuf_opt")]
	pub active_transcoding_config: Option<TranscodingConfig>,

	/// The active recording id
	pub active_recording_id: Option<Ulid>,

	/// The ingest bitrate
	pub ingest_bitrate: Option<i64>,

	/// The video output configs after transcoding
	#[from_row(from_fn = "protobuf_vec_opt")]
	pub video_output: Option<Vec<VideoConfig>>,

	/// The audio output configs after transcoding
	#[from_row(from_fn = "protobuf_vec_opt")]
	pub audio_output: Option<Vec<AudioConfig>>,

	/// Tags associated with the room
	#[from_row(from_fn = "json")]
	pub tags: HashMap<String, String>,
}

impl DatabaseTable for Room {
	const FRIENDLY_NAME: &'static str = "room";
	const NAME: &'static str = "rooms";
}

impl Room {
	pub fn into_proto(self) -> pb::scuffle::video::v1::types::Room {
		pb::scuffle::video::v1::types::Room {
			id: Some(self.id.into()),
			transcoding_config_id: self.transcoding_config_id.map(|id| id.into()),
			recording_config_id: self.recording_config_id.map(|id| id.into()),
			visibility: self.visibility.into(),
			created_at: self.id.timestamp_ms() as i64,
			updated_at: self.updated_at.timestamp_millis(),
			last_live_at: self.last_live_at.map(|t| t.timestamp_millis()),
			last_disconnected_at: self.last_disconnected_at.map(|t| t.timestamp_millis()),
			status: self.status.into(),
			audio_input: self.audio_input,
			video_input: self.video_input,
			audio_output: self.audio_output.unwrap_or_default(),
			video_output: self.video_output.unwrap_or_default(),
			active_recording_id: self.active_recording_id.map(|r| r.into()),
			active_connection_id: self.active_ingest_connection_id.map(|c| c.into()),
			tags: Some(self.tags.into()),
		}
	}
}
