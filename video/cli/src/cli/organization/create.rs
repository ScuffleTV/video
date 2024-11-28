use anyhow::Context;

use crate::cli::{Cli, Invokable};
use crate::invoker::request::OrganizationCreateRequest;
use crate::invoker::Invoker;

#[derive(clap::Args, Debug)]
pub struct Create {
	/// Path to the public key file
	#[clap(long, required = true)]
	name: String,

	/// The tags for the organization (JSON)
	#[clap(long, default_value = "{}")]
	tags: String,
}

impl Invokable for Create {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		let resp = invoker
			.invoke(OrganizationCreateRequest {
				name: self.name.clone(),
				tags: serde_json::from_str(&self.tags).context("failed to parse tags")?,
			})
			.await?;

		invoker.display(&resp)?;

		Ok(())
	}
}
