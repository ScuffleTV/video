#[derive(Debug, serde_derive::Deserialize, smart_default::SmartDefault)]
#[serde(default)]
struct Config {
	#[default = "baz"]
	foo: String,
	bar: i32,
	baz: bool,
}

fn main() {
	let config = scuffle_settings::parse_settings::<Config>(scuffle_settings::Options {
		cli: Some(scuffle_settings::cli!()),
		default_config_file: Some("config"),
		env_prefix: Some("APP"),
	});

	println!("{:#?}", config);
}
