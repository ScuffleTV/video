use anyhow::Context;

use super::S3Bucket;
use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;
#[derive(Debug, clap::Args)]
pub struct Create {
	/// The access key id for the s3 bucket
	#[clap(long, required = true)]
	access_key_id: String,

	/// The secret access key for the s3 bucket
	#[clap(long, required = true)]
	secret_access_key: String,

	/// The name of the s3 bucket
	#[clap(long, required = true)]
	name: String,

	/// The region of the s3 bucket
	#[clap(long, default_value = "us-east-1")]
	region: String,

	/// The endpoint of the s3 bucket
	#[clap(long)]
	endpoint: Option<String>,

	/// A public url for the s3 bucket
	#[clap(long)]
	public_url: Option<String>,

	/// The tags for the s3 bucket (JSON)
	#[clap(long, default_value = "{}")]
	tags: String,
}

impl Invokable for Create {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		let resp = invoker
			.invoke(pb::scuffle::video::v1::S3BucketCreateRequest {
				access_key_id: self.access_key_id.clone(),
				secret_access_key: self.secret_access_key.clone(),
				name: self.name.clone(),
				region: self.region.clone(),
				endpoint: self.endpoint.clone(),
				public_url: self.public_url.clone(),
				tags: Some(pb::scuffle::video::v1::types::Tags {
					tags: serde_json::from_str(&self.tags).context("failed to parse tags")?,
				}),
			})
			.await?;

		invoker.display(&S3Bucket::from_proto(resp.s3_bucket.unwrap_or_default()))?;

		Ok(())
	}
}
