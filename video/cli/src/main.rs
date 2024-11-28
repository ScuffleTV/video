use std::time::Duration;

use anyhow::Context as _;
use clap::Parser;
use cli::Invokable;
use invoker::Invoker;
use scuffle_utils::context::Context;
use scuffle_utils::prelude::FutureTimeout;

mod cli;
mod invoker;

#[tokio::main]
async fn main() {
	let (context, handler) = Context::new();

	if let Err(err) = start(context).await {
		eprintln!("{:#?}", err);
		std::process::exit(1);
	}

	handler.cancel().await;
}

async fn start(context: Context) -> anyhow::Result<()> {
	let cli = cli::Cli::parse();

	let mut invoker = Invoker::new(context, &cli)
		.timeout(Duration::from_secs(3))
		.await
		.context("failed to build invoker: timedout")??;

	cli.command
		.invoke(&mut invoker, &cli)
		.await
		.context("failed to invoke command")?;

	Ok(())
}
