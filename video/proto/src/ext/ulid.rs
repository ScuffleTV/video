use crate::scuffle::types::Ulid;

pub trait UlidExt {
	fn into_ulid(self) -> ulid::Ulid;
}

impl Ulid {
	pub const fn into_ulid(self) -> ulid::Ulid {
		ulid::Ulid((self.most_significant_ulid_bits as u128) << 64 | (self.least_significant_ulid_bits as u128))
	}

	pub const fn from_ulid(ulid: ulid::Ulid) -> Self {
		Self {
			most_significant_ulid_bits: (ulid.0 >> 64) as u64,
			least_significant_ulid_bits: ulid.0 as u64,
		}
	}

	pub const fn from_uuid(uuid: uuid::Uuid) -> Self {
		Self::from_ulid(ulid::Ulid::from_bytes(uuid.into_bytes()))
	}
}

impl Copy for Ulid {}

impl UlidExt for Option<Ulid> {
	#[inline]
	fn into_ulid(self) -> ulid::Ulid {
		match self {
			Some(ulid) => ulid.into_ulid(),
			None => ulid::Ulid::nil(),
		}
	}
}

impl From<uuid::Uuid> for Ulid {
	#[inline]
	fn from(uuid: uuid::Uuid) -> Self {
		Self::from_uuid(uuid)
	}
}

impl From<ulid::Ulid> for Ulid {
	#[inline]
	fn from(uuid: ulid::Ulid) -> Self {
		Self::from_ulid(uuid)
	}
}

impl Copy for crate::scuffle::video::v1::types::PlaybackSessionTarget {}

impl Copy for crate::scuffle::video::v1::types::playback_session_target::Target {}
