use anyhow::Context;
use ulid::Ulid;

use crate::cli::{Cli, Invokable};
use crate::invoker::request::OrganizationModifyRequest;
use crate::invoker::Invoker;

#[derive(clap::Args, Debug)]
pub struct Modify {
	/// The id of the organization to modify
	#[clap(long, required = true)]
	id: Ulid,

	/// Path to the public key file
	#[clap(long)]
	name: Option<String>,

	/// The tags for the organization (JSON)
	#[clap(long)]
	tags: Option<String>,
}

impl Invokable for Modify {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		if self.name.is_none() && self.tags.is_none() {
			anyhow::bail!("at least one of --name or --tags must be specified");
		}

		let resp = invoker
			.invoke(OrganizationModifyRequest {
				id: self.id,
				name: self.name.clone(),
				tags: self
					.tags
					.as_ref()
					.map(|tags| serde_json::from_str(tags))
					.transpose()
					.context("failed to parse tags")?,
			})
			.await?;

		invoker.display(&resp)?;

		Ok(())
	}
}
