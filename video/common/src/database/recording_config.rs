use std::collections::HashMap;

use pb::scuffle::video::v1::types::{RecordingLifecyclePolicy, Rendition as PbRendition};
use postgres_from_row::FromRow;
use scuffle_utils::database::{json, protobuf_vec};
use ulid::Ulid;

use super::{DatabaseTable, Rendition};

#[derive(Debug, Clone, Default, FromRow)]
pub struct RecordingConfig {
	/// The organization this recording config belongs to (primary key)
	pub organization_id: Ulid,
	/// A unique id for the recording config (primary key)
	pub id: Ulid,

	/// The renditions this recording config uses
	pub renditions: Vec<Rendition>,

	/// The lifecycle policies this recording config uses
	#[from_row(from_fn = "protobuf_vec")]
	pub lifecycle_policies: Vec<RecordingLifecyclePolicy>,

	/// The date and time the recording config was last updated
	pub updated_at: chrono::DateTime<chrono::Utc>,

	/// The S3 bucket this recording config uses
	pub s3_bucket_id: Ulid,

	/// Tags associated with the recording config
	#[from_row(from_fn = "json")]
	pub tags: HashMap<String, String>,
}

impl DatabaseTable for RecordingConfig {
	const FRIENDLY_NAME: &'static str = "recording config";
	const NAME: &'static str = "recording_configs";
}

impl RecordingConfig {
	pub fn into_proto(self) -> pb::scuffle::video::v1::types::RecordingConfig {
		let mut renditions: Vec<_> = self.renditions.into_iter().map(|r| PbRendition::from(r).into()).collect();
		renditions.sort();

		pb::scuffle::video::v1::types::RecordingConfig {
			id: Some(self.id.into()),
			renditions,
			s3_bucket_id: Some(self.s3_bucket_id.into()),
			lifecycle_policies: self.lifecycle_policies,
			created_at: self.id.timestamp_ms() as i64,
			updated_at: self.updated_at.timestamp_millis(),
			tags: Some(self.tags.into()),
		}
	}
}
