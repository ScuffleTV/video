pub mod config;
pub mod global;
pub mod service;

pub use config::{ConfigParser, EmptyConfig};
pub use global::Global;
pub use scuffle_bootstrap_derive::main;
pub use service::Service;

#[doc(hidden)]
pub mod prelude {
	pub use {anyhow, futures, scuffle_context, tokio};
}
