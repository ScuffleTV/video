use std::net::SocketAddr;
use std::time::Duration;

use binary_helper::config::TlsConfig;

#[derive(Debug, Clone, PartialEq, config::Config, serde::Deserialize)]
#[serde(default)]
pub struct RtmpConfig {
	/// The bind address for the RTMP server
	pub bind_address: SocketAddr,

	/// If we should use TLS for the RTMP server
	pub tls: Option<TlsConfig>,
}

impl Default for RtmpConfig {
	fn default() -> Self {
		Self {
			bind_address: "[::]:1935".to_string().parse().unwrap(),
			tls: None,
		}
	}
}

#[derive(Debug, Clone, PartialEq, config::Config, serde::Deserialize)]
#[serde(default)]
pub struct IngestConfig {
	/// Events stream name
	pub events_stream_name: String,

	/// NATS subject to send transcoder requests to
	pub transcoder_request_subject: String,

	/// The interval in to update the bitrate for a room
	pub bitrate_update_interval: Duration,

	/// The maximum time to wait for a transcoder
	pub transcoder_timeout: Duration,

	/// Max Bitrate for ingest
	pub max_bitrate: u64,

	/// Max bytes between keyframes
	pub max_bytes_between_keyframes: u64,

	/// Max time between keyframes
	pub max_time_between_keyframes: Duration,

	/// The config for the RTMP server
	pub rtmp: RtmpConfig,

	/// The address to advertise for the gRPC server which is used by
	/// transcoders to connect to
	pub grpc_advertise_address: String,
}

impl Default for IngestConfig {
	fn default() -> Self {
		Self {
			events_stream_name: "scuffle-video-events".to_string(),
			transcoder_request_subject: "scuffle-video-transcoder_requests".to_string(),
			bitrate_update_interval: Duration::from_secs(5),
			max_bitrate: 12000 * 1024,
			max_bytes_between_keyframes: 5 * 12000 * 1024 / 8,
			max_time_between_keyframes: Duration::from_secs(10),
			transcoder_timeout: Duration::from_secs(60),
			rtmp: Default::default(),
			grpc_advertise_address: "".to_string(),
		}
	}
}
