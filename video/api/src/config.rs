use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

use binary_helper::config::TlsConfig;

use crate::ratelimit::RateLimitResource;

#[derive(Debug, Clone, PartialEq, config::Config, serde::Deserialize)]
#[serde(default)]
pub struct ApiConfig {
	/// Bind Address
	pub bind_address: SocketAddr,

	/// The stream to use for recording delete events
	pub recording_delete_stream: String,

	/// The batch size for deleting recordings
	pub recording_delete_batch_size: usize,

	/// The events config
	pub events: EventsConfig,

	/// If we should use TLS
	pub tls: Option<TlsConfig>,

	/// The ratelimit rules
	pub rate_limit_rules: RatelimitRules,
}

#[derive(Debug, Clone, PartialEq, config::Config, serde::Deserialize)]
#[serde(default)]
pub struct EventsConfig {
	/// Event stream name
	pub stream_name: String,

	/// The maximum age of an event before it is deleted
	pub nats_stream_message_max_age: Duration,

	/// The duration to hold a lease on an event
	pub nats_stream_message_lease_duration: Duration,

	/// The maximum delay to wait for events on the fetch api request
	pub fetch_request_max_delay: Duration,

	/// The minimum delay to wait for events on the fetch api request
	pub fetch_request_min_delay: Duration,

	/// The maximum number of events to return on the fetch api request
	pub fetch_request_max_messages: usize,

	/// The minimum number of events to return on the fetch api request
	pub fetch_request_min_messages: usize,
}

impl Default for EventsConfig {
	fn default() -> Self {
		Self {
			stream_name: "scuffle-video-events".to_string(),
			nats_stream_message_max_age: Duration::from_secs(60 * 60 * 24 * 7), // 7 days
			nats_stream_message_lease_duration: Duration::from_secs(60),        // 60 seconds
			fetch_request_max_delay: Duration::from_secs(60),                   // 60 seconds
			fetch_request_min_delay: Duration::from_secs(5),                    // 5 seconds
			fetch_request_max_messages: 1000,
			fetch_request_min_messages: 1,
		}
	}
}

#[derive(Debug, Default, Clone, PartialEq, config::Config, serde::Deserialize)]
#[serde(default)]
pub struct RatelimitRules {
	/// The default rules
	pub default: RatelimitRule,

	/// Banned and exceeded rules
	pub banned_exceeded: RatelimitBannedExceededRules,

	/// The custom rules
	#[config(cli(skip), env(skip))]
	pub rules: HashMap<RateLimitResource, RatelimitRule>,
}

#[derive(Debug, Clone, PartialEq, config::Config, serde::Deserialize)]
#[serde(default)]
pub struct RatelimitBannedExceededRules {
	/// The number of exceeded requests before the user is banned
	pub exceeded_limit: u32,
	/// The amount of time before exceeded is reset in seconds
	pub exceeded_reset_seconds: u32,
	/// The amount of time before the user is unbanned in seconds
	pub banned_reset_seconds: u32,
}

impl Default for RatelimitBannedExceededRules {
	fn default() -> Self {
		Self {
			exceeded_limit: 1000,
			exceeded_reset_seconds: 180,
			banned_reset_seconds: 900,
		}
	}
}

#[derive(Debug, Clone, PartialEq, config::Config, serde::Deserialize)]
#[serde(default)]
pub struct RatelimitRule {
	/// The cost of the request
	pub cost: u32,
	/// The allowed quota for the duration
	pub quota: u32,
	/// How often the quota is reset in seconds
	pub quota_reset_seconds: u32,
	/// Failed restore cost
	pub failed_cost: u32,
}

impl Default for RatelimitRule {
	fn default() -> Self {
		Self {
			cost: 10,
			quota: 1000,
			quota_reset_seconds: 30,
			failed_cost: 1,
		}
	}
}

impl Default for ApiConfig {
	fn default() -> Self {
		Self {
			bind_address: "[::]:9080".to_string().parse().unwrap(),
			tls: None,
			events: EventsConfig::default(),
			recording_delete_stream: "scuffle-video-recording_delete".to_string(),
			recording_delete_batch_size: 1000,
			rate_limit_rules: RatelimitRules::default(),
		}
	}
}
