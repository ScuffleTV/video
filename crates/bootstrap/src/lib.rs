pub mod config;
pub mod global;
pub mod service;

#[cfg(feature = "signal")]
pub mod signal;

pub use scuffle_bootstrap_derive::main;

#[doc(hidden)]
pub mod prelude {
	#[cfg(feature = "settings")]
	pub use scuffle_settings;
	#[cfg(feature = "signal")]
	pub use scuffle_signal;
	pub use {anyhow, futures, scuffle_context, tokio};
}
