use std::collections::HashMap;

use postgres_from_row::FromRow;
use scuffle_utils::database::json;
use ulid::Ulid;

use super::DatabaseTable;

#[derive(Debug, Clone, Default, FromRow)]
pub struct S3Bucket {
	/// The organization this S3 bucket belongs to (primary key)
	pub organization_id: Ulid,
	/// A unique id for the S3 bucket (primary key)
	pub id: Ulid,

	/// The name of the S3 bucket
	pub name: String,

	/// The region the S3 bucket is in
	pub region: String,

	/// The custom endpoint for the S3 bucket
	pub endpoint: Option<String>,

	/// The access key id for the S3 bucket
	pub access_key_id: String,

	/// The secret access key for the S3 bucket
	pub secret_access_key: String,

	/// The public url for the S3 bucket
	pub public_url: Option<String>,

	/// Whether or not the S3 bucket is managed by Scuffle
	pub managed: bool,

	/// Tags associated with the S3 bucket
	#[from_row(from_fn = "json")]
	pub tags: HashMap<String, String>,
}

impl DatabaseTable for S3Bucket {
	const FRIENDLY_NAME: &'static str = "s3 bucket";
	const NAME: &'static str = "s3_buckets";
}

impl S3Bucket {
	pub fn into_proto(self) -> pb::scuffle::video::v1::types::S3Bucket {
		pb::scuffle::video::v1::types::S3Bucket {
			id: Some(self.id.into()),
			name: self.name,
			region: self.region,
			endpoint: self.endpoint,
			access_key_id: self.access_key_id,
			public_url: self.public_url,
			managed: self.managed,
			tags: Some(self.tags.into()),
		}
	}
}
