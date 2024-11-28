#[derive(Debug, Clone, Copy)]
pub enum IngestError {
	KeyframeBitrateDistance(u64, u64),
	BitrateLimit(u64, u64),
	VideoDemux,
	AudioDemux,
	MetadataDemux,
	Mux,
	KeyframeTimeLimit(u64),
	NoTranscoderAvailable,
	FailedToUpdateBitrate,
	FailedToSubscribe,
	IngestShutdown,
	RtmpConnectionError,
	RtmpConnectionTimeout,
	DisconnectRequested,
	SubscriptionClosedUnexpectedly,
	FailedToRequestTranscoder,
	FailedToUpdateRoom,
}

impl std::fmt::Display for IngestError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::KeyframeBitrateDistance(a, b) => write!(
				f,
				"I01: Keyframe byte distance too large, the bitrate is too high for the keyframe interval: {}KB >= {}KB",
				a / 1024,
				b / 1024
			),
			Self::BitrateLimit(a, b) => write!(
				f,
				"I02: Bitrate limit reached, the bitrate is too high: {}Kbps >= {}Kbps",
				a / 1024,
				b / 1024
			),
			Self::VideoDemux => write!(f, "I03: Video Demux Error"),
			Self::AudioDemux => write!(f, "I04: Audio Demux Error"),
			Self::MetadataDemux => write!(f, "I05: Metadata Demux Error"),
			Self::Mux => write!(f, "I06: Mux Error"),
			Self::KeyframeTimeLimit(a) => write!(
				f,
				"I07: Keyframe time distance too large, the keyframe interval is larger than: {}s",
				a
			),
			Self::NoTranscoderAvailable => write!(f, "I08: No transcoder available"),
			Self::FailedToUpdateBitrate => write!(f, "I09: Failed to update bitrate"),
			Self::FailedToSubscribe => write!(f, "I10: Failed to subscribe"),
			Self::IngestShutdown => write!(f, "I11: Ingest shutdown"),
			Self::RtmpConnectionError => write!(f, "I12: RTMP connection error"),
			Self::RtmpConnectionTimeout => write!(f, "I13: RTMP connection timeout"),
			Self::DisconnectRequested => write!(f, "I14: Disconnect requested"),
			Self::SubscriptionClosedUnexpectedly => {
				write!(f, "I15: Subscription closed unexpectedly")
			}
			Self::FailedToRequestTranscoder => write!(f, "I16: Failed to request transcoder"),
			Self::FailedToUpdateRoom => write!(f, "I17: Failed to update room"),
		}
	}
}
