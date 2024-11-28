use std::io::Read;

use anyhow::Context;

use super::PlaybackKeyPair;
use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;

#[derive(Debug, clap::Args)]
pub struct Create {
	/// Path to the public key file
	#[clap(long, required = true)]
	public_key: String,

	/// The tags for the playback key pair (JSON)
	#[clap(long, default_value = "{}")]
	tags: String,
}

impl Invokable for Create {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		let public_key = if self.public_key == "-" {
			let mut buf = String::new();
			std::io::stdin()
				.read_to_string(&mut buf)
				.context("failed to read public key from stdin")?;
			buf
		} else {
			std::fs::read_to_string(&self.public_key)
				.with_context(|| format!("failed to read public key from {}", self.public_key))?
		};

		let resp = invoker
			.invoke(pb::scuffle::video::v1::PlaybackKeyPairCreateRequest {
				public_key,
				tags: Some(pb::scuffle::video::v1::types::Tags {
					tags: serde_json::from_str(&self.tags).context("failed to parse tags")?,
				}),
			})
			.await?;

		invoker.display(&PlaybackKeyPair::from_proto(resp.playback_key_pair.unwrap_or_default()))?;

		Ok(())
	}
}
