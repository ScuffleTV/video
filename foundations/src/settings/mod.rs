#[cfg(feature = "cli")]
pub mod cli;

#[cfg(feature = "macros")]
pub use scuffle_foundations_macros::{auto_settings, Settings};

#[derive(Debug, Clone)]
pub struct SettingsParser<S> {
	root: Option<toml::Value>,
	_marker: std::marker::PhantomData<S>,
}

impl<S> SettingsParser<S> {
	pub fn new(default: &S) -> Result<Self, toml::ser::Error>
	where
		S: serde::Serialize,
	{
		Ok(Self {
			root: Some(toml::Value::try_from(default)?),
			_marker: std::marker::PhantomData,
		})
	}

	fn merge(&mut self, incoming: toml::Value) {
		let root = self.root.take().unwrap();
		self.root = Some(Self::merge_loop(root, incoming));
	}

	fn merge_loop(root: toml::Value, incoming: toml::Value) -> toml::Value {
		match (root, incoming) {
			(toml::Value::Table(mut first_map), toml::Value::Table(second_map)) => {
				for (key, value) in second_map {
					let combined_value = if let Some(existing_value) = first_map.remove(&key) {
						Self::merge_loop(existing_value, value)
					} else {
						value
					};
					first_map.insert(key, combined_value);
				}

				toml::Value::Table(first_map)
			}
			(_, second) => second,
		}
	}

	pub fn merge_str(&mut self, s: &str) -> Result<(), toml::de::Error> {
		let incoming = toml::from_str(s)?;
		self.merge(incoming);
		Ok(())
	}

	pub fn parse(self) -> Result<S, toml::de::Error>
	where
		for<'de> S: serde::Deserialize<'de>,
	{
		self.root.unwrap().try_into()
	}
}

mod traits;

pub use traits::{Settings, Wrapped};

/// Converts a settings struct to a YAML string including doc comments.
/// If you want to provide doc comments for keys use to_yaml_string_with_docs.
pub fn to_docs_string<T: serde::Serialize + Settings>(settings: &T) -> Result<String, toml::ser::Error> {
	toml::to_string_pretty(settings)
}

// type CowStr = Cow<'static, str>;
// type DocMap = HashMap<Vec<CowStr>, Cow<'static, [CowStr]>>;
