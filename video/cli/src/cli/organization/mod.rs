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
	/// Get organizations
	Get(get::Get),

	/// Create an organization
	Create(create::Create),

	/// Modify organization
	Modify(modify::Modify),

	/// Delete organizations
	Delete(delete::Delete),

	/// Tag organizations
	Tag(tag::Tag),

	/// Untag organizations
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
