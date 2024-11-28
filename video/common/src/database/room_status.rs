use postgres_types::{FromSql, ToSql};

#[derive(Debug, Default, ToSql, FromSql, Clone, Copy, PartialEq)]
#[postgres(name = "room_status")]
pub enum RoomStatus {
	#[postgres(name = "OFFLINE")]
	#[default]
	Offline,
	#[postgres(name = "WAITING_FOR_TRANSCODER")]
	WaitingForTranscoder,
	#[postgres(name = "READY")]
	Ready,
}

impl From<RoomStatus> for i32 {
	fn from(value: RoomStatus) -> Self {
		pb::scuffle::video::v1::types::RoomStatus::from(value) as i32
	}
}

impl From<RoomStatus> for pb::scuffle::video::v1::types::RoomStatus {
	fn from(value: RoomStatus) -> Self {
		match value {
			RoomStatus::Offline => pb::scuffle::video::v1::types::RoomStatus::Offline,
			RoomStatus::WaitingForTranscoder => pb::scuffle::video::v1::types::RoomStatus::WaitingForTranscoder,
			RoomStatus::Ready => pb::scuffle::video::v1::types::RoomStatus::Ready,
		}
	}
}

impl From<pb::scuffle::video::v1::types::RoomStatus> for RoomStatus {
	fn from(value: pb::scuffle::video::v1::types::RoomStatus) -> Self {
		match value {
			pb::scuffle::video::v1::types::RoomStatus::Offline => RoomStatus::Offline,
			pb::scuffle::video::v1::types::RoomStatus::WaitingForTranscoder => RoomStatus::WaitingForTranscoder,
			pb::scuffle::video::v1::types::RoomStatus::Ready => RoomStatus::Ready,
		}
	}
}
