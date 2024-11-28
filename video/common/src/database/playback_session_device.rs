use postgres_types::{FromSql, ToSql};

#[derive(Debug, ToSql, FromSql, Default, Clone, Copy, PartialEq)]
#[postgres(name = "playback_session_device")]
pub enum PlaybackSessionDevice {
	#[postgres(name = "UNKNOWN")]
	#[default]
	Unknown,
}

impl From<PlaybackSessionDevice> for pb::scuffle::video::v1::types::playback_session::Device {
	fn from(value: PlaybackSessionDevice) -> Self {
		match value {
			PlaybackSessionDevice::Unknown => Self::UnknownDevice,
		}
	}
}
