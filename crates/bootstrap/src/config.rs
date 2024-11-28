pub trait ConfigParser: Sized {
	fn parse() -> impl std::future::Future<Output = anyhow::Result<Self>>;
}

impl ConfigParser for () {
	#[inline(always)]
	fn parse() -> impl std::future::Future<Output = anyhow::Result<Self>> {
		std::future::ready(Ok(()))
	}
}

pub struct EmptyConfig;

impl ConfigParser for EmptyConfig {
	#[inline(always)]
	fn parse() -> impl std::future::Future<Output = anyhow::Result<Self>> {
		std::future::ready(Ok(EmptyConfig))
	}
}

/// A macro to create a config parser from a CLI struct
/// This macro will automatically parse the CLI struct into the given type
/// using the `scuffle-settings` crate
#[cfg(feature = "settings")]
#[macro_export]
macro_rules! cli_config {
	($ty:ty) => {
		impl $crate::config::ConfigParser for $ty {
			async fn parse() -> anyhow::Result<Self> {
				use $crate::prelude::anyhow::Context;

				$crate::prelude::scuffle_settings::parse_settings(
					$crate::prelude::scuffle_settings::Options::builder()
						.cli($crate::prelude::scuffle_settings::cli!())
						.build(),
				)
				.context("config")
			}
		}
	};
}
