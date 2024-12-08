use clap::Parser;
use cmd::Commands;

mod cmd;
mod utils;

#[derive(Debug, clap::Parser)]
#[command(
	name = "cargo xtask",
	bin_name = "cargo xtask",
	about = "A utility for running commands in the workspace"
)]
struct Cli {
	#[command(subcommand)]
	command: Commands,
}

fn main() -> anyhow::Result<()> {
	Cli::parse().command.run()
}
