use std::collections::HashMap;
use std::time::Duration;

use binary_helper::config::TlsConfig;

#[derive(Debug, Clone, PartialEq, config::Config, serde::Deserialize)]
#[serde(default)]
pub struct TranscoderConfig {
	/// Events stream name
	pub events_stream_name: String,

	/// The name of the transcoder requests queue to use
	pub transcoder_request_subject: String,

	/// The NATS KV bucket to use for metadata
	pub metadata_kv_store: String,

	/// The NATS ObjectStore bucket to use for media
	pub media_ob_store: String,

	/// The target segment length
	pub min_segment_duration: Duration,

	/// The target part length
	pub target_part_duration: Duration,

	/// The maximum part length
	pub max_part_duration: Duration,

	/// The TLS config to use when connecting to ingest
	pub ingest_tls: Option<TlsConfig>,

	/// The number of segments to keep in the playlist
	pub playlist_segments: usize,

	/// The interval to take screenshots at
	pub screenshot_interval: Duration,

	/// The encoder to use for h264
	pub h264_encoder: Option<String>,

	/// H264 encoder options
	#[config(cli(skip), env(skip))]
	pub h264_encoder_options: HashMap<String, String>,
}

impl Default for TranscoderConfig {
	fn default() -> Self {
		Self {
			events_stream_name: "scuffle-video-events".to_string(),
			transcoder_request_subject: "scuffle-video-transcoder_requests".to_string(),
			metadata_kv_store: "scuffle-video-transcoder_metadata".to_string(),
			media_ob_store: "scuffle-video-transcoder_media".to_string(),
			min_segment_duration: Duration::from_secs(2),
			target_part_duration: Duration::from_millis(250),
			max_part_duration: Duration::from_millis(500),
			screenshot_interval: Duration::from_secs(5),
			ingest_tls: None,
			playlist_segments: 5,
			h264_encoder: Some("libx264".to_string()),
			h264_encoder_options: vec![("tune".into(), "zerolatency".into())].into_iter().collect(),
		}
	}
}
