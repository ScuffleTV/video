use ulid::Ulid;

use crate::cli::{Cli, Invokable};
use crate::invoker::request::OrganizationUntagRequest;
use crate::invoker::Invoker;

#[derive(clap::Args, Debug)]
pub struct Untag {
	/// The ids of the organizations to untag
	#[clap(long, required = true)]
	id: Ulid,

	/// The tags to remove from the organization
	#[clap(long, value_parser, num_args = 1.., value_delimiter = ' ', required = true)]
	tags: Vec<String>,
}

impl Invokable for Untag {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		let resp = invoker
			.invoke(OrganizationUntagRequest {
				id: self.id,
				tags: self.tags.clone(),
			})
			.await?;

		invoker.display(&resp)?;

		Ok(())
	}
}
