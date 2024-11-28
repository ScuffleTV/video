use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;
mod ack;
mod fetch;

#[derive(Debug, clap::Subcommand)]
pub enum Commands {
	/// Fetch events
	Fetch(fetch::Fetch),

	/// Acknowledge events
	Ack(ack::Ack),
}

impl Invokable for Commands {
	async fn invoke(&self, invoker: &mut Invoker, args: &Cli) -> anyhow::Result<()> {
		match self {
			Self::Fetch(cmd) => cmd.invoke(invoker, args).await,
			Self::Ack(cmd) => cmd.invoke(invoker, args).await,
		}
	}
}
