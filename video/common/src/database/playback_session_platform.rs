use postgres_types::{FromSql, ToSql};

#[derive(Debug, ToSql, FromSql, Default, Clone, Copy, PartialEq)]
#[postgres(name = "playback_session_platform")]
pub enum PlaybackSessionPlatform {
	#[postgres(name = "UNKNOWN")]
	#[default]
	Unknown,
}

impl From<PlaybackSessionPlatform> for pb::scuffle::video::v1::types::playback_session::Platform {
	fn from(value: PlaybackSessionPlatform) -> Self {
		match value {
			PlaybackSessionPlatform::Unknown => Self::UnknownPlatform,
		}
	}
}
