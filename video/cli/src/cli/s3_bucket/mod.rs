use std::collections::HashMap;

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
	/// Get s3 buckets
	Get(get::Get),

	/// Create an s3 bucket
	Create(create::Create),

	/// Modify s3 bucket
	Modify(modify::Modify),

	/// Delete s3 buckets
	Delete(delete::Delete),

	/// Tag s3 buckets
	Tag(tag::Tag),

	/// Untag s3 buckets
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
pub struct S3Bucket {
	id: ulid::Ulid,
	name: String,
	access_key_id: String,
	region: String,
	endpoint: Option<String>,
	public_url: Option<String>,
	managed: bool,
	#[serde(skip_serializing_if = "HashMap::is_empty")]
	tags: HashMap<String, String>,
}

impl S3Bucket {
	fn from_proto(pb: pb::scuffle::video::v1::types::S3Bucket) -> Self {
		Self {
			id: pb.id.into_ulid(),
			name: pb.name,
			access_key_id: pb.access_key_id,
			region: pb.region,
			endpoint: pb.endpoint,
			public_url: pb.public_url,
			managed: pb.managed,
			tags: pb.tags.map(|tags| tags.tags).unwrap_or_default(),
		}
	}
}
