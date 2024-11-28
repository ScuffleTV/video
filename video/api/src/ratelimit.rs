use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Copy, Hash, Eq)]
pub enum RateLimitResource {
	AccessTokenGet,
	AccessTokenCreate,
	AccessTokenDelete,
	AccessTokenTag,
	AccessTokenUntag,

	EventsFetch,
	EventsAck,

	PlaybackKeyPairGet,
	PlaybackKeyPairCreate,
	PlaybackKeyPairModify,
	PlaybackKeyPairDelete,
	PlaybackKeyPairTag,
	PlaybackKeyPairUntag,

	PlaybackSessionGet,
	PlaybackSessionRevoke,
	PlaybackSessionCount,

	RecordingConfigGet,
	RecordingConfigCreate,
	RecordingConfigModify,
	RecordingConfigDelete,
	RecordingConfigTag,
	RecordingConfigUntag,

	RecordingGet,
	RecordingModify,
	RecordingDelete,
	RecordingTag,
	RecordingUntag,

	RoomGet,
	RoomCreate,
	RoomModify,
	RoomDelete,
	RoomDisconnect,
	RoomResetKey,
	RoomTag,
	RoomUntag,

	S3BucketGet,
	S3BucketCreate,
	S3BucketModify,
	S3BucketDelete,
	S3BucketTag,
	S3BucketUntag,

	TranscodingConfigGet,
	TranscodingConfigCreate,
	TranscodingConfigModify,
	TranscodingConfigDelete,
	TranscodingConfigTag,
	TranscodingConfigUntag,
}

impl RateLimitResource {
	#[must_use]
	pub const fn name(&self) -> &'static str {
		match self {
			Self::AccessTokenGet => "access_token:get",
			Self::AccessTokenCreate => "access_token:create",
			Self::AccessTokenDelete => "access_token:delete",
			Self::AccessTokenTag => "access_token:tag",
			Self::AccessTokenUntag => "access_token:untag",

			Self::EventsFetch => "events:subscribe",
			Self::EventsAck => "events:ack",

			Self::PlaybackKeyPairGet => "playback_key_pair:get",
			Self::PlaybackKeyPairCreate => "playback_key_pair:create",
			Self::PlaybackKeyPairModify => "playback_key_pair:modify",
			Self::PlaybackKeyPairDelete => "playback_key_pair:delete",
			Self::PlaybackKeyPairTag => "playback_key_pair:tag",
			Self::PlaybackKeyPairUntag => "playback_key_pair:untag",

			Self::PlaybackSessionGet => "playback_session:get",
			Self::PlaybackSessionRevoke => "playback_session:revoke",
			Self::PlaybackSessionCount => "playback_session:count",

			Self::RecordingConfigGet => "recording_config:get",
			Self::RecordingConfigCreate => "recording_config:create",
			Self::RecordingConfigModify => "recording_config:modify",
			Self::RecordingConfigDelete => "recording_config:delete",
			Self::RecordingConfigTag => "recording_config:tag",
			Self::RecordingConfigUntag => "recording_config:untag",

			Self::RecordingGet => "recording:get",
			Self::RecordingModify => "recording:modify",
			Self::RecordingDelete => "recording:delete",
			Self::RecordingTag => "recording:tag",
			Self::RecordingUntag => "recording:untag",

			Self::RoomGet => "room:get",
			Self::RoomCreate => "room:create",
			Self::RoomModify => "room:modify",
			Self::RoomDelete => "room:delete",
			Self::RoomDisconnect => "room:disconnect",
			Self::RoomResetKey => "room:reset_key",
			Self::RoomTag => "room:tag",
			Self::RoomUntag => "room:untag",

			Self::S3BucketGet => "s3_bucket:get",
			Self::S3BucketCreate => "s3_bucket:create",
			Self::S3BucketModify => "s3_bucket:modify",
			Self::S3BucketDelete => "s3_bucket:delete",
			Self::S3BucketTag => "s3_bucket:tag",
			Self::S3BucketUntag => "s3_bucket:untag",

			Self::TranscodingConfigGet => "transcoding_config:get",
			Self::TranscodingConfigCreate => "transcoding_config:create",
			Self::TranscodingConfigModify => "transcoding_config:modify",
			Self::TranscodingConfigDelete => "transcoding_config:delete",
			Self::TranscodingConfigTag => "transcoding_config:tag",
			Self::TranscodingConfigUntag => "transcoding_config:untag",
		}
	}
}

impl FromStr for RateLimitResource {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"access_token:get" => Ok(Self::AccessTokenGet),
			"access_token:create" => Ok(Self::AccessTokenCreate),
			"access_token:delete" => Ok(Self::AccessTokenDelete),
			"access_token:tag" => Ok(Self::AccessTokenTag),
			"access_token:untag" => Ok(Self::AccessTokenUntag),

			"events:subscribe" => Ok(Self::EventsFetch),

			"playback_key_pair:get" => Ok(Self::PlaybackKeyPairGet),
			"playback_key_pair:create" => Ok(Self::PlaybackKeyPairCreate),
			"playback_key_pair:modify" => Ok(Self::PlaybackKeyPairModify),
			"playback_key_pair:delete" => Ok(Self::PlaybackKeyPairDelete),
			"playback_key_pair:tag" => Ok(Self::PlaybackKeyPairTag),
			"playback_key_pair:untag" => Ok(Self::PlaybackKeyPairUntag),
			"playback_session:get" => Ok(Self::PlaybackSessionGet),
			"playback_session:revoke" => Ok(Self::PlaybackSessionRevoke),
			"playback_session:count" => Ok(Self::PlaybackSessionCount),

			"recording_config:get" => Ok(Self::RecordingConfigGet),
			"recording_config:create" => Ok(Self::RecordingConfigCreate),
			"recording_config:modify" => Ok(Self::RecordingConfigModify),
			"recording_config:delete" => Ok(Self::RecordingConfigDelete),
			"recording_config:tag" => Ok(Self::RecordingConfigTag),
			"recording_config:untag" => Ok(Self::RecordingConfigUntag),
			"recording:get" => Ok(Self::RecordingGet),
			"recording:modify" => Ok(Self::RecordingModify),
			"recording:delete" => Ok(Self::RecordingDelete),
			"recording:tag" => Ok(Self::RecordingTag),
			"recording:untag" => Ok(Self::RecordingUntag),

			"room:get" => Ok(Self::RoomGet),
			"room:create" => Ok(Self::RoomCreate),
			"room:modify" => Ok(Self::RoomModify),
			"room:delete" => Ok(Self::RoomDelete),
			"room:disconnect" => Ok(Self::RoomDisconnect),
			"room:reset_key" => Ok(Self::RoomResetKey),
			"room:tag" => Ok(Self::RoomTag),
			"room:untag" => Ok(Self::RoomUntag),

			"s3_bucket:get" => Ok(Self::S3BucketGet),
			"s3_bucket:create" => Ok(Self::S3BucketCreate),
			"s3_bucket:modify" => Ok(Self::S3BucketModify),
			"s3_bucket:delete" => Ok(Self::S3BucketDelete),
			"s3_bucket:tag" => Ok(Self::S3BucketTag),
			"s3_bucket:untag" => Ok(Self::S3BucketUntag),

			"transcoding_config:get" => Ok(Self::TranscodingConfigGet),
			"transcoding_config:create" => Ok(Self::TranscodingConfigCreate),
			"transcoding_config:modify" => Ok(Self::TranscodingConfigModify),
			"transcoding_config:delete" => Ok(Self::TranscodingConfigDelete),
			"transcoding_config:tag" => Ok(Self::TranscodingConfigTag),
			"transcoding_config:untag" => Ok(Self::TranscodingConfigUntag),

			_ => Err(()),
		}
	}
}

impl std::fmt::Display for RateLimitResource {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.name())
	}
}

impl config::Config for RateLimitResource {
	fn graph() -> std::sync::Arc<config::KeyGraph> {
		String::graph()
	}
}

impl<'de> serde::Deserialize<'de> for RateLimitResource {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de>,
	{
		let s = String::deserialize(deserializer)?;
		Self::from_str(&s).map_err(|_| serde::de::Error::custom("invalid rate limit resource"))
	}
}
