use std::collections::HashMap;

use chrono::TimeZone;
use pb::ext::UlidExt;
use ulid::Ulid;
use video_api::api::RequiredScope;

use super::{Cli, Invokable};
use crate::invoker::Invoker;

mod create;
mod delete;
mod get;
mod tag;
mod untag;

#[derive(Debug, clap::Subcommand)]
pub enum Commands {
	/// Get access tokens
	Get(get::Get),

	/// Create an access token
	Create(create::Create),

	/// Delete access tokens
	Delete(delete::Delete),

	/// Tag access tokens
	Tag(tag::Tag),

	/// Untag access tokens
	Untag(untag::Untag),
}

impl Invokable for Commands {
	async fn invoke(&self, invoker: &mut Invoker, args: &Cli) -> anyhow::Result<()> {
		match self {
			Self::Get(cmd) => cmd.invoke(invoker, args).await,
			Self::Create(cmd) => cmd.invoke(invoker, args).await,
			Self::Delete(cmd) => cmd.invoke(invoker, args).await,
			Self::Tag(cmd) => cmd.invoke(invoker, args).await,
			Self::Untag(cmd) => cmd.invoke(invoker, args).await,
		}
	}
}

#[derive(Debug, serde::Serialize)]
pub struct AccessToken {
	pub id: Ulid,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub secret: Option<String>,
	pub scopes: Vec<String>,
	#[serde(skip_serializing_if = "HashMap::is_empty")]
	pub tags: HashMap<String, String>,
	pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
	pub last_used_at: Option<chrono::DateTime<chrono::Utc>>,
	pub created_at: chrono::DateTime<chrono::Utc>,
	pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl AccessToken {
	pub fn from_proto(proto: pb::scuffle::video::v1::types::AccessToken, secret: Option<String>) -> Self {
		Self {
			id: proto.id.into_ulid(),
			secret,
			tags: proto.tags.map(|tags| tags.tags).unwrap_or_default(),
			created_at: chrono::Utc.timestamp_millis_opt(proto.created_at).unwrap(),
			updated_at: chrono::Utc.timestamp_millis_opt(proto.updated_at).unwrap(),
			expires_at: proto.expires_at.map(|ts| chrono::Utc.timestamp_millis_opt(ts).unwrap()),
			last_used_at: proto.last_used_at.map(|ts| chrono::Utc.timestamp_millis_opt(ts).unwrap()),
			scopes: RequiredScope::from(proto.scopes).string_vec(),
		}
	}
}
