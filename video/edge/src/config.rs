use std::net::SocketAddr;

use binary_helper::config::TlsConfig;

#[derive(Debug, Clone, PartialEq, config::Config, serde::Deserialize)]
#[serde(default)]
pub struct EdgeConfig {
	/// The address to bind to
	pub bind_address: SocketAddr,

	/// TLS configuration
	pub tls: Option<TlsConfig>,

	/// The session key to use for signing session tokens
	pub session_key: String,

	/// The segment key to use for signing segment tokens
	pub media_key: String,

	/// The name of the key value store to use for metadata
	pub metadata_kv_store: String,

	/// The name of the object store to use for media
	pub media_ob_store: String,

	/// The real IP mode to use (default: socket address)
	pub ip_header_mode: Option<String>,

	/// The number of hops to trust for the ip header (default: all)
	pub ip_header_trusted_hops: Option<usize>,
}

impl Default for EdgeConfig {
	fn default() -> Self {
		Self {
			bind_address: "0.0.0.0:10100".parse().unwrap(),
			tls: None,
			media_key: "media_key".to_string(),
			session_key: "session_key".to_string(),
			metadata_kv_store: "scuffle-video-transcoder_metadata".to_string(),
			media_ob_store: "scuffle-video-transcoder_media".to_string(),
			ip_header_mode: None,
			ip_header_trusted_hops: None,
		}
	}
}
