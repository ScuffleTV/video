use pb::scuffle::video::v1::types::SearchOptions;
use ulid::Ulid;

use super::S3Bucket;
use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;

#[derive(Debug, clap::Args)]
pub struct Get {
	/// The ids of the s3 buckets to get
	#[clap(long, value_parser, num_args = 1.., value_delimiter = ' ')]
	ids: Vec<Ulid>,

	/// The maximum number of s3 buckets to get
	#[clap(long, default_value = "100")]
	limit: usize,

	/// The ID after which to start getting s3 buckets
	#[clap(long)]
	after: Option<Ulid>,

	/// The tags to filter s3 buckets by (JSON)
	#[clap(long, default_value = "{}")]
	tags: String,

	/// Reverse the order of the s3 buckets
	#[clap(long)]
	reverse: bool,
}

impl Invokable for Get {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		let resp = invoker
			.invoke(pb::scuffle::video::v1::S3BucketGetRequest {
				ids: self.ids.iter().copied().map(|id| id.into()).collect(),
				search_options: Some(SearchOptions {
					limit: self.limit as _,
					after_id: self.after.map(Into::into),
					tags: Some(pb::scuffle::video::v1::types::Tags {
						tags: serde_json::from_str(&self.tags)?,
					}),
					reverse: self.reverse,
				}),
			})
			.await?;

		invoker.display_array(&resp.s3_buckets.into_iter().map(S3Bucket::from_proto).collect::<Vec<_>>())?;

		Ok(())
	}
}
