use anyhow::Context;

pub fn metadata() -> anyhow::Result<cargo_metadata::Metadata> {
    cargo_metadata::MetadataCommand::new().exec().context("cargo metadata")
}

pub fn cargo_cmd() -> std::process::Command {
    std::process::Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()))
}

pub fn comma_delimited(features: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    let mut string = String::new();
    for feature in features {
        if !string.is_empty() {
            string.push(',');
        }
        string.push_str(feature.as_ref());
    }
    string
}
