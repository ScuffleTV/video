use std::io::Read;

use anyhow::Context;
use ulid::Ulid;

use super::PlaybackKeyPair;
use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;

#[derive(Debug, clap::Args)]
pub struct Modify {
	/// The id of the playback key pair to modify
	#[clap(long, required = true)]
	id: Ulid,

	/// Path to the public key file
	#[clap(long)]
	public_key: Option<String>,

	/// The tags for the playback key pair (JSON)
	#[clap(long)]
	tags: Option<String>,
}

impl Invokable for Modify {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		if self.public_key.is_none() && self.tags.is_none() {
			anyhow::bail!("at least one of --public-key or --tags must be specified");
		}

		let public_key = if let Some(public_key) = &self.public_key {
			Some(if public_key == "-" {
				let mut buf = String::new();
				std::io::stdin()
					.read_to_string(&mut buf)
					.context("failed to read public key from stdin")?;
				buf
			} else {
				std::fs::read_to_string(public_key)
					.with_context(|| format!("failed to read public key from {}", public_key))?
			})
		} else {
			None
		};

		let resp = invoker
			.invoke(pb::scuffle::video::v1::PlaybackKeyPairModifyRequest {
				id: Some(self.id.into()),
				public_key,
				tags: self
					.tags
					.as_ref()
					.map(|tags| {
						anyhow::Ok(pb::scuffle::video::v1::types::Tags {
							tags: serde_json::from_str(tags).context("failed to parse tags")?,
						})
					})
					.transpose()?,
			})
			.await?;

		invoker.display(&PlaybackKeyPair::from_proto(resp.playback_key_pair.unwrap_or_default()))?;

		Ok(())
	}
}
