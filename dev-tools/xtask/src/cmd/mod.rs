use anyhow::Context;

mod power_set;

#[derive(Debug, Clone, clap::Subcommand)]
pub enum Commands {
	#[clap(alias = "powerset")]
	PowerSet(power_set::PowerSet),
}

impl Commands {
	pub fn run(self) -> anyhow::Result<()> {
		match self {
			Commands::PowerSet(cmd) => cmd.run().context("power set"),
		}
	}
}
