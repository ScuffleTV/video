use std::collections::HashMap;

use chrono::{TimeZone, Utc};
use pb::ext::UlidExt;
use ulid::Ulid;

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
	/// Get playback key pairs
	Get(get::Get),

	/// Create an playback key pair
	Create(create::Create),

	/// Modify playback key pair
	Modify(modify::Modify),

	/// Delete playback key pairs
	Delete(delete::Delete),

	/// Tag playback key pairs
	Tag(tag::Tag),

	/// Untag playback key pairs
	Untag(untag::Untag),
}

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
pub struct PlaybackKeyPair {
	pub id: Ulid,
	pub fingerprint: String,
	pub created_at: chrono::DateTime<chrono::Utc>,
	pub updated_at: chrono::DateTime<chrono::Utc>,
	#[serde(skip_serializing_if = "HashMap::is_empty")]
	pub tags: HashMap<String, String>,
}

impl PlaybackKeyPair {
	pub fn from_proto(pb: pb::scuffle::video::v1::types::PlaybackKeyPair) -> Self {
		Self {
			id: pb.id.into_ulid(),
			fingerprint: pb.fingerprint,
			created_at: Utc.timestamp_millis_opt(pb.created_at).unwrap(),
			updated_at: Utc.timestamp_millis_opt(pb.updated_at).unwrap(),
			tags: pb.tags.map(|tags| tags.tags).unwrap_or_default(),
		}
	}
}
