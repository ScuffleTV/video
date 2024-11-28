use std::collections::HashMap;

use postgres_from_row::FromRow;
use scuffle_utils::database::json;
use ulid::Ulid;

use super::DatabaseTable;

#[derive(Debug, Clone, Default, FromRow)]
pub struct PlaybackKeyPair {
	/// The organization this playback key pair belongs to (primary key)
	pub organization_id: Ulid,
	/// A unique id for the playback key pair (primary key)
	pub id: Ulid,

	/// The public key (in PEM format) for the playback key pair
	pub public_key: Vec<u8>,

	/// The fingerprint of the public key (SHA-256, hex-encoded)
	pub fingerprint: String,

	/// The date and time the playback key pair was last updated
	pub updated_at: chrono::DateTime<chrono::Utc>,

	/// Tags associated with the playback key pair
	#[from_row(from_fn = "json")]
	pub tags: HashMap<String, String>,
}

impl DatabaseTable for PlaybackKeyPair {
	const FRIENDLY_NAME: &'static str = "playback key pair";
	const NAME: &'static str = "playback_key_pairs";
}

impl PlaybackKeyPair {
	pub fn into_proto(self) -> pb::scuffle::video::v1::types::PlaybackKeyPair {
		pb::scuffle::video::v1::types::PlaybackKeyPair {
			id: Some(self.id.into()),
			fingerprint: self.fingerprint,
			created_at: self.id.timestamp_ms() as i64,
			updated_at: self.updated_at.timestamp_millis(),
			tags: Some(self.tags.into()),
		}
	}
}
