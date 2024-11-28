use std::collections::HashMap;

use chrono::{TimeZone, Utc};
use pb::ext::UlidExt;
use ulid::Ulid;

use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;

mod delete;
mod get;
mod modify;
mod tag;
mod untag;

#[derive(Debug, clap::Subcommand)]
pub enum Commands {
	/// Get recordings
	Get(get::Get),

	/// Modify recording
	Modify(modify::Modify),

	/// Delete recordings
	Delete(delete::Delete),

	/// Tag recordings
	Tag(tag::Tag),

	/// Untag recordings
	Untag(untag::Untag),
}

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
	Public,
	Private,
}

impl Invokable for Commands {
	async fn invoke(&self, invoker: &mut Invoker, args: &Cli) -> anyhow::Result<()> {
		match self {
			Self::Get(cmd) => cmd.invoke(invoker, args).await,
			Self::Modify(cmd) => cmd.invoke(invoker, args).await,
			Self::Delete(cmd) => cmd.invoke(invoker, args).await,
			Self::Tag(cmd) => cmd.invoke(invoker, args).await,
			Self::Untag(cmd) => cmd.invoke(invoker, args).await,
		}
	}
}

#[derive(Debug, serde::Serialize)]
struct Recording {
	pub id: Ulid,
	pub room_id: Option<Ulid>,
	pub recording_config_id: Option<Ulid>,
	pub s3_bucket_id: Ulid,
	pub renditions: Vec<String>,
	pub visibility: String,
	pub byte_size: i64,
	pub duration: f32,
	pub created_at: chrono::DateTime<chrono::Utc>,
	pub updated_at: chrono::DateTime<chrono::Utc>,
	pub ended_at: Option<chrono::DateTime<chrono::Utc>>,
	pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
	#[serde(skip_serializing_if = "HashMap::is_empty")]
	pub tags: HashMap<String, String>,
}

impl Recording {
	fn from_proto(pb: pb::scuffle::video::v1::types::Recording) -> Self {
		Recording {
			byte_size: pb.byte_size,
			created_at: Utc.timestamp_millis_opt(pb.created_at).unwrap(),
			updated_at: Utc.timestamp_millis_opt(pb.updated_at).unwrap(),
			deleted_at: pb.deleted_at.map(|t| Utc.timestamp_millis_opt(t).unwrap()),
			ended_at: pb.ended_at.map(|t| Utc.timestamp_millis_opt(t).unwrap()),
			duration: pb.duration,
			id: pb.id.into_ulid(),
			s3_bucket_id: pb.s3_bucket_id.into_ulid(),
			recording_config_id: pb.recording_config_id.map(|id| id.into_ulid()),
			room_id: pb.room_id.map(|id| id.into_ulid()),
			visibility: pb.visibility().as_str_name().to_string(),
			renditions: pb.renditions().map(|r| r.as_str_name().to_string()).collect(),
			tags: pb.tags.map(|t| t.tags).unwrap_or_default(),
		}
	}
}
