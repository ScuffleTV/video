use postgres_from_row::FromRow;
use ulid::Ulid;

use super::DatabaseTable;

#[derive(Debug, Clone, Default, FromRow)]
pub struct SessionTokenRevoke {
	/// The organization id this revoke belongs to (primary key)
	pub organization_id: Ulid,
	/// The room id this revoke belongs to (primary key) (either this or
	/// `recording_id` will be set)
	pub room_id: Option<Ulid>,
	/// The recording id this revoke belongs to (primary key) (either this or
	/// `room_id` will be set)
	pub recording_id: Option<Ulid>,
	/// The user id this revoke is for (primary key)
	pub user_id: Option<String>,
	/// The sso id this revoke is for (primary key)
	pub sso_id: Option<String>,

	/// The time this revoke is no longer valid
	pub expires_at: chrono::DateTime<chrono::Utc>,
}

impl DatabaseTable for SessionTokenRevoke {
	const FRIENDLY_NAME: &'static str = "session token revoke";
	const NAME: &'static str = "session_token_revokes";
}
