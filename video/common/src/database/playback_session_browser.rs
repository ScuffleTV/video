use postgres_types::{FromSql, ToSql};

#[derive(Debug, ToSql, FromSql, Default, Clone, Copy, PartialEq)]
#[postgres(name = "playback_session_browser")]
pub enum PlaybackSessionBrowser {
	#[postgres(name = "UNKNOWN")]
	#[default]
	Unknown,
}

impl From<PlaybackSessionBrowser> for pb::scuffle::video::v1::types::playback_session::Browser {
	fn from(browser: PlaybackSessionBrowser) -> Self {
		match browser {
			PlaybackSessionBrowser::Unknown => Self::UnknownBrowser,
		}
	}
}
