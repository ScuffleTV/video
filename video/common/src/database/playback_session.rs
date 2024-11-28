use std::net::IpAddr;

use pb::scuffle::video::v1::types::{playback_session, playback_session_target, PlaybackSessionTarget};
use postgres_from_row::FromRow;
use ulid::Ulid;

use super::playback_session_browser::PlaybackSessionBrowser;
use super::playback_session_device::PlaybackSessionDevice;
use super::playback_session_platform::PlaybackSessionPlatform;
use super::DatabaseTable;

#[derive(Debug, Clone, FromRow)]
pub struct PlaybackSession {
	/// The organization this playback session belongs to (primary key)
	pub organization_id: Ulid,
	/// A unique id for the playback session (primary key)
	pub id: Ulid,

	/// The room this playback session belongs to (either this or `recording_id`
	/// will be set)
	pub room_id: Option<Ulid>,

	/// The recording this playback session belongs to (either this or `room_id`
	/// will be set)
	pub recording_id: Option<Ulid>,

	/// The user id of the playback session (this is set if the token issued a
	/// token with a user id)
	pub user_id: Option<String>,

	/// The playback key pair id used to issue the token (set if the token was
	/// issued)
	pub playback_key_pair_id: Option<Ulid>,

	/// The date and time the playback session was issued (set if the token was
	/// issued)
	pub issued_at: Option<chrono::DateTime<chrono::Utc>>,

	/// The date and time the playback session expires
	pub expires_at: chrono::DateTime<chrono::Utc>,

	/// The ip address of the client that used the playback session
	pub ip_address: IpAddr,

	/// The user agent of the client that used the playback session
	pub user_agent: Option<String>,

	/// The referer of the client that used the playback session
	pub referer: Option<String>,

	/// The origin of the client that used the playback session
	pub origin: Option<String>,

	/// The device of the client that used the playback session
	pub device: PlaybackSessionDevice,

	/// The platform of the client that used the playback session
	pub platform: PlaybackSessionPlatform,

	/// The browser of the client that used the playback session
	pub browser: PlaybackSessionBrowser,

	/// The player version of the client that used the playback session
	pub player_version: Option<String>,
}

impl DatabaseTable for PlaybackSession {
	const FRIENDLY_NAME: &'static str = "playback session";
	const NAME: &'static str = "playback_sessions";
}

impl PlaybackSession {
	pub fn into_proto(self) -> pb::scuffle::video::v1::types::PlaybackSession {
		pb::scuffle::video::v1::types::PlaybackSession {
			id: Some(self.id.into()),
			target: Some(PlaybackSessionTarget {
				target: Some(match self.room_id {
					Some(room_id) => playback_session_target::Target::RoomId(room_id.into()),
					None => playback_session_target::Target::RecordingId(self.recording_id.unwrap().into()),
				}),
			}),
			user_id: self.user_id,
			playback_key_pair_id: self.playback_key_pair_id.map(|id| id.into()),
			issued_at: self.issued_at.map(|dt| dt.timestamp_millis()),
			created_at: self.id.timestamp_ms() as i64,
			last_active_at: (self.expires_at - chrono::Duration::minutes(10)).timestamp_millis(),
			ip_address: self.ip_address.to_string(),
			user_agent: self.user_agent,
			referer: self.referer,
			origin: self.origin,
			device: playback_session::Device::from(self.device).into(),
			platform: playback_session::Platform::from(self.platform).into(),
			browser: playback_session::Browser::from(self.browser).into(),
			player_version: self.player_version,
		}
	}
}
