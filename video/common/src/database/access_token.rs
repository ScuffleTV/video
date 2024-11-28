use std::collections::HashMap;

use chrono::Utc;
use pb::scuffle::video::v1::types::AccessTokenScope;
use postgres_from_row::FromRow;
use scuffle_utils::database::{json, protobuf_vec};
use ulid::Ulid;

use super::DatabaseTable;

#[derive(Debug, Clone, Default, FromRow)]
pub struct AccessToken {
	/// The organization this access token is for (primary key)
	pub organization_id: Ulid,
	/// Unique id of the access token (primary key)
	pub id: Ulid,

	/// The secret token used to access the API
	pub secret_token: Ulid,

	/// The scopes associated with this access token
	#[from_row(from_fn = "protobuf_vec")]
	pub scopes: Vec<AccessTokenScope>,

	/// The last time the access token was used
	pub last_active_at: Option<chrono::DateTime<Utc>>,

	/// The last time the token was modified
	pub updated_at: chrono::DateTime<Utc>,

	/// The time the token expires
	pub expires_at: Option<chrono::DateTime<Utc>>,

	/// Tags associated with the access token
	#[from_row(from_fn = "json")]
	pub tags: HashMap<String, String>,
}

impl DatabaseTable for AccessToken {
	const FRIENDLY_NAME: &'static str = "access token";
	const NAME: &'static str = "access_tokens";
}

impl AccessToken {
	pub fn into_proto(self) -> pb::scuffle::video::v1::types::AccessToken {
		pb::scuffle::video::v1::types::AccessToken {
			id: Some(self.id.into()),
			created_at: self.id.timestamp_ms() as i64,
			updated_at: self.updated_at.timestamp_millis(),
			expires_at: self.expires_at.map(|t| t.timestamp_millis()),
			last_used_at: self.last_active_at.map(|t| t.timestamp_millis()),
			scopes: self.scopes,
			tags: Some(self.tags.into()),
		}
	}
}
