#![doc = include_str!("../README.md")]

use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
	#[error(transparent)]
	Config(#[from] config::ConfigError),
	#[cfg(feature = "cli")]
	#[error(transparent)]
	Clap(#[from] clap::Error),
}

/// A struct used to define how the CLI should be generated
#[derive(Debug, Clone)]
pub struct Cli {
	/// The name of the program
	pub name: &'static str,

	/// The version of the program
	pub version: &'static str,

	/// The about of the program
	pub about: &'static str,

	/// The author of the program
	pub author: &'static str,

	/// The arguments to add to the CLI
	pub argv: Vec<String>,
}

/// A macro to create a CLI struct
/// This macro will automatically set the name, version, about, and author from
/// the environment variables at compile time
#[macro_export]
macro_rules! cli {
	() => {
		$crate::cli!(std::env::args().collect())
	};
	($args:expr) => {
		$crate::Cli {
			name: env!("CARGO_BIN_NAME"),
			version: env!("CARGO_PKG_VERSION"),
			about: env!("CARGO_PKG_DESCRIPTION"),
			author: env!("CARGO_PKG_AUTHORS"),
			argv: $args,
		}
	};
}

#[derive(Debug, Clone, Copy)]
struct FormatWrapper;

use std::borrow::Cow;

#[cfg(not(feature = "templates"))]
fn template_text<'a>(
	text: &'a str,
	_: &config::FileFormat,
) -> Result<Cow<'a, str>, Box<dyn std::error::Error + Send + Sync>> {
	Ok(Cow::Borrowed(text))
}

#[cfg(feature = "templates")]
fn template_text<'a>(
	text: &'a str,
	_: &config::FileFormat,
) -> Result<Cow<'a, str>, Box<dyn std::error::Error + Send + Sync>> {
	use minijinja::syntax::SyntaxConfig;

	let mut env = minijinja::Environment::new();

	env.add_global("env", std::env::vars().collect::<std::collections::HashMap<_, _>>());
	env.set_syntax(
		SyntaxConfig::builder()
			.block_delimiters("{%", "%}")
			.variable_delimiters("${{", "}}")
			.comment_delimiters("{#", "#}")
			.build()
			.unwrap(),
	);

	Ok(Cow::Owned(env.template_from_str(text).unwrap().render(())?))
}

impl config::Format for FormatWrapper {
	fn parse(
		&self,
		uri: Option<&String>,
		text: &str,
	) -> Result<config::Map<String, config::Value>, Box<dyn std::error::Error + Send + Sync>> {
		match uri.and_then(|s| Path::new(s.as_str()).extension()).and_then(|s| s.to_str()) {
			#[cfg(feature = "toml")]
			Some("toml") => config::FileFormat::Toml.parse(uri, template_text(text, &config::FileFormat::Toml)?.as_ref()),
			#[cfg(not(feature = "toml"))]
			Some("toml") => {
				return Err(Box::new(std::io::Error::new(
					std::io::ErrorKind::InvalidData,
					format!("toml support is not enabled, consider building with the `toml` feature enabled"),
				)))
			}
			#[cfg(feature = "json")]
			Some("json") => config::FileFormat::Json.parse(uri, template_text(text, &config::FileFormat::Json)?.as_ref()),
			#[cfg(not(feature = "json"))]
			Some("json") => {
				return Err(Box::new(std::io::Error::new(
					std::io::ErrorKind::InvalidData,
					format!("json support is not enabled, consider building with the `json` feature enabled"),
				)))
			}
			#[cfg(feature = "yaml")]
			Some("yaml") | Some("yml") => {
				config::FileFormat::Yaml.parse(uri, template_text(text, &config::FileFormat::Yaml)?.as_ref())
			}
			#[cfg(not(feature = "yaml"))]
			Some("yaml") | Some("yml") => {
				return Err(Box::new(std::io::Error::new(
					std::io::ErrorKind::InvalidData,
					format!("yaml support is not enabled, consider building with the `yaml` feature enabled"),
				)))
			}
			#[cfg(feature = "json5")]
			Some("json5") => config::FileFormat::Json5.parse(uri, template_text(text, &config::FileFormat::Json5)?.as_ref()),
			#[cfg(not(feature = "json5"))]
			Some("json5") => {
				return Err(Box::new(std::io::Error::new(
					std::io::ErrorKind::InvalidData,
					format!("json5 support is not enabled, consider building with the `json5` feature enabled"),
				)))
			}
			#[cfg(feature = "ini")]
			Some("ini") => config::FileFormat::Ini.parse(uri, template_text(text, &config::FileFormat::Ini)?.as_ref()),
			#[cfg(not(feature = "ini"))]
			Some("ini") => {
				return Err(Box::new(std::io::Error::new(
					std::io::ErrorKind::InvalidData,
					format!("ini support is not enabled, consider building with the `ini` feature enabled"),
				)))
			}
			#[cfg(feature = "ron")]
			Some("ron") => config::FileFormat::Ron.parse(uri, template_text(text, &config::FileFormat::Ron)?.as_ref()),
			#[cfg(not(feature = "ron"))]
			Some("ron") => {
				return Err(Box::new(std::io::Error::new(
					std::io::ErrorKind::InvalidData,
					format!("ron support is not enabled, consider building with the `ron` feature enabled"),
				)))
			}
			_ => {
				let formats: &[config::FileFormat] = &[
					#[cfg(feature = "toml")]
					config::FileFormat::Toml,
					#[cfg(feature = "json")]
					config::FileFormat::Json,
					#[cfg(feature = "yaml")]
					config::FileFormat::Yaml,
					#[cfg(feature = "json5")]
					config::FileFormat::Json5,
					#[cfg(feature = "ini")]
					config::FileFormat::Ini,
					#[cfg(feature = "ron")]
					config::FileFormat::Ron,
				];

				for format in formats {
					if let Ok(map) = format.parse(uri, template_text(text, format)?.as_ref()) {
						return Ok(map);
					}
				}

				Err(Box::new(std::io::Error::new(
					std::io::ErrorKind::InvalidData,
					format!("No supported format found for file: {:?}", uri),
				)))
			}
		}
	}
}

impl config::FileStoredFormat for FormatWrapper {
	fn file_extensions(&self) -> &'static [&'static str] {
		&[
			#[cfg(feature = "toml")]
			"toml",
			#[cfg(feature = "json")]
			"json",
			#[cfg(feature = "yaml")]
			"yaml",
			#[cfg(feature = "yaml")]
			"yml",
			#[cfg(feature = "json5")]
			"json5",
			#[cfg(feature = "ini")]
			"ini",
			#[cfg(feature = "ron")]
			"ron",
		]
	}
}

#[derive(Debug, Clone)]
pub struct Options {
	/// The CLI options
	#[cfg(feature = "cli")]
	pub cli: Option<Cli>,
	/// The default config file name (loaded if no other files are specified)
	pub default_config_file: Option<&'static str>,
	/// Environment variables prefix
	pub env_prefix: Option<&'static str>,
}

impl Default for Options {
	fn default() -> Self {
		Self {
			#[cfg(feature = "cli")]
			cli: None,
			default_config_file: Some("config"),
			env_prefix: Some("APP"),
		}
	}
}

pub fn parse_settings<T: serde::de::DeserializeOwned>(options: Options) -> Result<T, ConfigError> {
	let mut config = config::Config::builder();

	#[allow(unused_mut)]
	let mut added_files = false;

	#[cfg(feature = "cli")]
	if let Some(cli) = options.cli {
		let command = clap::Command::new(cli.name)
			.version(cli.version)
			.about(cli.about)
			.author(cli.author)
			.bin_name(cli.name)
			.arg(
				clap::Arg::new("config")
					.short('c')
					.long("config")
					.value_name("FILE")
					.help("Path to configuration file(s)")
					.action(clap::ArgAction::Append),
			)
			.arg(
				clap::Arg::new("overrides")
					.long("override")
					.short('o')
					.alias("set")
					.help("Provide an override for a configuration value, in the format KEY=VALUE")
					.action(clap::ArgAction::Append),
			);

		let matches = command.get_matches_from(cli.argv);

		if let Some(config_files) = matches.get_many::<String>("config") {
			for path in config_files {
				config = config.add_source(config::File::new(path, FormatWrapper));
				added_files = true;
			}
		}

		if let Some(overrides) = matches.get_many::<String>("overrides") {
			for ov in overrides {
				let (key, value) = ov.split_once('=').ok_or_else(|| {
					clap::Error::raw(
						clap::error::ErrorKind::InvalidValue,
						"Override must be in the format KEY=VALUE",
					)
				})?;

				config = config.set_override(key, value)?;
			}
		}
	}

	if !added_files {
		if let Some(default_config_file) = options.default_config_file {
			config = config.add_source(config::File::new(default_config_file, FormatWrapper).required(false));
		}
	}

	if let Some(env_prefix) = options.env_prefix {
		config = config.add_source(config::Environment::with_prefix(env_prefix));
	}

	Ok(config.build()?.try_deserialize()?)
}

#[doc(hidden)]
#[cfg(feature = "bootstrap")]
pub mod macros {
	pub use {anyhow, scuffle_bootstrap};
}

/// A macro to create a config parser from a CLI struct
/// This macro will automatically parse the CLI struct into the given type
/// using the `scuffle-settings` crate
#[cfg(feature = "bootstrap")]
#[macro_export]
macro_rules! bootstrap {
	($ty:ty) => {
		impl $crate::macros::scuffle_bootstrap::config::ConfigParser for $ty {
			async fn parse() -> $crate::macros::anyhow::Result<Self> {
				$crate::macros::anyhow::Context::context(
					$crate::parse_settings($crate::Options {
						cli: Some($crate::cli!()),
						..::std::default::Default::default()
					}),
					"config",
				)
			}
		}
	};
}
