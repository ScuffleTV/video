use ulid::Ulid;

use crate::cli::{Cli, Invokable};
use crate::invoker::request::OrganizationDeleteRequest;
use crate::invoker::Invoker;

#[derive(clap::Args, Debug)]
pub struct Delete {
	/// The ids of the organizations to delete
	#[clap(long, value_parser, num_args = 1.., value_delimiter = ' ', required = true)]
	ids: Vec<Ulid>,
}

impl Invokable for Delete {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		let resp = invoker.invoke(OrganizationDeleteRequest { ids: self.ids.clone() }).await?;

		invoker.display(&resp)?;

		Ok(())
	}
}
