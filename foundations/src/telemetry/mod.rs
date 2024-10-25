#[cfg(feature = "env-filter")]
mod env_filter;

#[cfg(feature = "env-filter")]
pub use env_filter::{EnvFilter, EnvFilterBuilder};

#[cfg(feature = "opentelemetry")]
pub mod opentelemetry;

#[cfg(feature = "pprof-cpu")]
pub mod pprof;

#[cfg(feature = "metrics")]
pub mod metrics;

#[cfg(feature = "settings")]
pub mod settings;

#[cfg(feature = "logging")]
pub mod logging;

#[cfg(feature = "telemetry-server")]
pub mod server;

#[cfg(not(feature = "env-filter"))]
type Underlying = tracing::level_filters::LevelFilter;

#[cfg(feature = "env-filter")]
type Underlying = crate::telemetry::EnvFilter;

#[derive(Debug)]
pub struct LevelFilter(Underlying);

impl LevelFilter {
	#[cfg(not(feature = "env-filter"))]
	pub fn new(level: &str) -> Self {
		match level.to_lowercase().as_str() {
			"trace" => Self(Underlying::from(tracing::Level::TRACE)),
			"debug" => Self(Underlying::from(tracing::Level::DEBUG)),
			"info" => Self(Underlying::from(tracing::Level::INFO)),
			"warn" => Self(Underlying::from(tracing::Level::WARN)),
			"error" => Self(Underlying::from(tracing::Level::ERROR)),
			_ => {
				eprintln!("log level '{level}' is not recognized, defaulting to 'info'");
				Self(Underlying::from(tracing::Level::INFO))
			}
		}
	}

	#[cfg(feature = "env-filter")]
	pub fn new(level: &str) -> Self {
		Self(Underlying::new(level))
	}

	pub fn filter(self) -> Underlying {
		self.0
	}
}

impl Default for LevelFilter {
	fn default() -> Self {
		Self::new("info")
	}
}
