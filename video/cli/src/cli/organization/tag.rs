use anyhow::Context;
use ulid::Ulid;

use crate::cli::{Cli, Invokable};
use crate::invoker::request::OrganizationTagRequest;
use crate::invoker::Invoker;

#[derive(clap::Args, Debug)]
pub struct Tag {
	/// The ids of the organizations to tag
	#[clap(long, required = true)]
	id: Ulid,

	/// The tags to add to the organization (JSON)
	#[clap(long, required = true)]
	tags: String,
}

impl Invokable for Tag {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		let resp = invoker
			.invoke(OrganizationTagRequest {
				id: self.id,
				tags: serde_json::from_str(&self.tags).context("failed to parse tags")?,
			})
			.await?;

		invoker.display(&resp)
	}
}
