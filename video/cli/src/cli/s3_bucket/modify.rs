use anyhow::Context;
use ulid::Ulid;

use super::S3Bucket;
use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;

#[derive(Debug, clap::Args)]
pub struct Modify {
	/// The id of the s3 bucket to modify
	#[clap(long, required = true)]
	id: Ulid,

	/// The access key id for the s3 bucket
	#[clap(long)]
	access_key_id: Option<String>,

	/// The secret access key for the s3 bucket
	#[clap(long)]
	secret_access_key: Option<String>,

	/// The name of the s3 bucket
	#[clap(long)]
	name: Option<String>,

	/// The region of the s3 bucket
	#[clap(long)]
	region: Option<String>,

	/// The endpoint of the s3 bucket
	#[clap(long)]
	endpoint: Option<String>,

	/// A public url for the s3 bucket
	#[clap(long)]
	public_url: Option<String>,

	/// The tags for the s3 bucket (JSON)
	#[clap(long)]
	tags: Option<String>,
}

impl Invokable for Modify {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		if self.access_key_id.is_none()
			&& self.secret_access_key.is_none()
			&& self.name.is_none()
			&& self.region.is_none()
			&& self.endpoint.is_none()
			&& self.public_url.is_none()
			&& self.tags.is_none()
		{
			anyhow::bail!(
				"at least one flag must be set, --access-key-id, --secret-access-key, --name, --region, --endpoint, --public-url, or --tags"
			);
		}

		let resp = invoker
			.invoke(pb::scuffle::video::v1::S3BucketModifyRequest {
				id: Some(self.id.into()),
				access_key_id: self.access_key_id.clone(),
				secret_access_key: self.secret_access_key.clone(),
				name: self.name.clone(),
				region: self.region.clone(),
				endpoint: self.endpoint.clone(),
				public_url: self.public_url.clone(),
				tags: self
					.tags
					.as_ref()
					.map(|tags| {
						anyhow::Ok(pb::scuffle::video::v1::types::Tags {
							tags: serde_json::from_str(tags).context("failed to parse tags")?,
						})
					})
					.transpose()?,
			})
			.await?;

		invoker.display(&S3Bucket::from_proto(resp.s3_bucket.unwrap_or_default()))?;

		Ok(())
	}
}
