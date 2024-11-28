use std::collections::HashMap;

use pb::scuffle::video::v1::types::Rendition as PbRendition;
use postgres_from_row::FromRow;
use scuffle_utils::database::json;
use ulid::Ulid;

use super::{DatabaseTable, Rendition};

#[derive(Debug, Clone, Default, FromRow)]
pub struct TranscodingConfig {
	/// The organization this transcoding config belongs to (primary key)
	pub organization_id: Ulid,
	/// A unique id for the transcoding config (primary key)
	pub id: Ulid,

	/// The renditions this transcoding config uses
	pub renditions: Vec<Rendition>,

	/// The date and time the transcoding config was last updated
	pub updated_at: chrono::DateTime<chrono::Utc>,

	/// Tags associated with the transcoding config
	#[from_row(from_fn = "json")]
	pub tags: HashMap<String, String>,
}

impl DatabaseTable for TranscodingConfig {
	const FRIENDLY_NAME: &'static str = "transcoding config";
	const NAME: &'static str = "transcoding_configs";
}

impl TranscodingConfig {
	pub fn into_proto(self) -> pb::scuffle::video::v1::types::TranscodingConfig {
		let mut renditions: Vec<_> = self.renditions.into_iter().map(|r| PbRendition::from(r).into()).collect();
		renditions.sort();

		pb::scuffle::video::v1::types::TranscodingConfig {
			id: Some(self.id.into()),
			renditions,
			created_at: self.id.timestamp_ms() as i64,
			updated_at: self.updated_at.timestamp_micros(),
			tags: Some(self.tags.into()),
		}
	}
}
