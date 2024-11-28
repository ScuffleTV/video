use std::collections::HashMap;

use chrono::{TimeZone, Utc};
use pb::ext::UlidExt;

use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;
mod create;
mod delete;
mod get;
mod modify;
mod tag;
mod untag;

#[derive(Debug, clap::Subcommand)]
pub enum Commands {
	/// Get transcoding configs
	Get(get::Get),

	/// Create an transcoding config
	Create(create::Create),

	/// Modify transcoding config
	Modify(modify::Modify),

	/// Delete transcoding configs
	Delete(delete::Delete),

	/// Tag transcoding configs
	Tag(tag::Tag),

	/// Untag transcoding configs
	Untag(untag::Untag),
}

pub use super::recording_config::Rendition;

impl Invokable for Commands {
	async fn invoke(&self, invoker: &mut Invoker, args: &Cli) -> anyhow::Result<()> {
		match self {
			Self::Get(cmd) => cmd.invoke(invoker, args).await,
			Self::Create(cmd) => cmd.invoke(invoker, args).await,
			Self::Modify(cmd) => cmd.invoke(invoker, args).await,
			Self::Delete(cmd) => cmd.invoke(invoker, args).await,
			Self::Tag(cmd) => cmd.invoke(invoker, args).await,
			Self::Untag(cmd) => cmd.invoke(invoker, args).await,
		}
	}
}

#[derive(Debug, serde::Serialize)]
pub struct TranscodingConfig {
	id: ulid::Ulid,
	renditions: Vec<String>,
	created_at: chrono::DateTime<chrono::Utc>,
	updated_at: chrono::DateTime<chrono::Utc>,
	#[serde(skip_serializing_if = "HashMap::is_empty")]
	tags: HashMap<String, String>,
}

impl TranscodingConfig {
	pub fn from_proto(proto: pb::scuffle::video::v1::types::TranscodingConfig) -> Self {
		Self {
			id: proto.id.into_ulid(),
			renditions: proto.renditions().map(|r| r.as_str_name().to_string()).collect(),
			tags: proto.tags.map(|tags| tags.tags).unwrap_or_default(),
			created_at: Utc.timestamp_millis_opt(proto.created_at).unwrap(),
			updated_at: Utc.timestamp_millis_opt(proto.updated_at).unwrap(),
		}
	}
}
