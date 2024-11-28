pub mod config;
pub mod global;
pub mod service;
pub mod signals;

pub use scuffle_bootstrap_derive::main;

#[doc(hidden)]
pub mod prelude {
	#[cfg(feature = "settings")]
	pub use scuffle_settings;
	pub use {anyhow, futures, scuffle_context, scuffle_signal, tokio};
}
