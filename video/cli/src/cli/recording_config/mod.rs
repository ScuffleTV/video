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
	/// Get recording configs
	Get(get::Get),

	/// Create an recording config
	Create(create::Create),

	/// Modify recording config
	Modify(modify::Modify),

	/// Delete recording configs
	Delete(delete::Delete),

	/// Tag recording configs
	Tag(tag::Tag),

	/// Untag recording configs
	Untag(untag::Untag),
}

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rendition {
	VideoSource,
	VideoHd,
	VideoSd,
	VideoLd,
	AudioSource,
}

impl From<Rendition> for i32 {
	fn from(rendition: Rendition) -> Self {
		match rendition {
			Rendition::VideoSource => pb::scuffle::video::v1::types::Rendition::VideoSource as i32,
			Rendition::VideoHd => pb::scuffle::video::v1::types::Rendition::VideoHd as i32,
			Rendition::VideoSd => pb::scuffle::video::v1::types::Rendition::VideoSd as i32,
			Rendition::VideoLd => pb::scuffle::video::v1::types::Rendition::VideoLd as i32,
			Rendition::AudioSource => pb::scuffle::video::v1::types::Rendition::AudioSource as i32,
		}
	}
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
pub struct RecordingConfig {
	pub id: Ulid,
	pub renditions: Vec<String>,
	pub lifecycle_policies: Vec<LifecyclePolicy>,
	pub s3_bucket_id: Ulid,
	pub created_at: chrono::DateTime<chrono::Utc>,
	pub updated_at: chrono::DateTime<chrono::Utc>,
	#[serde(skip_serializing_if = "HashMap::is_empty")]
	pub tags: HashMap<String, String>,
}

impl RecordingConfig {
	pub fn from_proto(pb: pb::scuffle::video::v1::types::RecordingConfig) -> Self {
		Self {
			id: pb.id.into_ulid(),
			renditions: pb.renditions().map(|r| r.as_str_name().to_string()).collect(),
			lifecycle_policies: pb
				.lifecycle_policies
				.iter()
				.map(|p| LifecyclePolicy {
					after_days: p.after_days,
					action: p.action().as_str_name().to_string(),
					renditions: p.renditions().map(|r| r.as_str_name().to_string()).collect(),
				})
				.collect(),
			s3_bucket_id: pb.s3_bucket_id.into_ulid(),
			created_at: Utc.timestamp_millis_opt(pb.created_at).unwrap(),
			updated_at: Utc.timestamp_millis_opt(pb.updated_at).unwrap(),
			tags: pb.tags.map(|tags| tags.tags).unwrap_or_default(),
		}
	}
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct LifecyclePolicy {
	pub after_days: i32,
	pub action: String,
	pub renditions: Vec<String>,
}

impl TryFrom<LifecyclePolicy> for pb::scuffle::video::v1::types::RecordingLifecyclePolicy {
	type Error = anyhow::Error;

	fn try_from(value: LifecyclePolicy) -> Result<Self, Self::Error> {
		Ok(Self {
			after_days: value.after_days,
			action: match value.action.to_lowercase().as_str() {
				"delete" => pb::scuffle::video::v1::types::recording_lifecycle_policy::Action::Delete as i32,
				_ => anyhow::bail!("invalid lifecycle policy action: {}", value.action),
			},
			renditions: value
				.renditions
				.iter()
				.map(|r| match r.to_lowercase().as_str() {
					"video_source" => Ok(pb::scuffle::video::v1::types::Rendition::VideoSource as i32),
					"video_hd" => Ok(pb::scuffle::video::v1::types::Rendition::VideoHd as i32),
					"video_sd" => Ok(pb::scuffle::video::v1::types::Rendition::VideoSd as i32),
					"video_ld" => Ok(pb::scuffle::video::v1::types::Rendition::VideoLd as i32),
					"audio_source" => Ok(pb::scuffle::video::v1::types::Rendition::AudioSource as i32),
					_ => anyhow::bail!("invalid rendition: {}", r),
				})
				.collect::<Result<_, _>>()?,
		})
	}
}
