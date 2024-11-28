use anyhow::Context;
use pb::scuffle::video::v1::types::{SearchOptions, Tags};
use pb::scuffle::video::v1::PlaybackKeyPairGetRequest;
use ulid::Ulid;

use super::PlaybackKeyPair;
use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;

#[derive(Debug, clap::Args)]
pub struct Get {
	/// The ids of the playback key pairs to get
	#[clap(long, value_parser, num_args = 1.., value_delimiter = ' ')]
	ids: Vec<Ulid>,

	/// The maximum number of playback key pairs to get
	#[clap(long, default_value = "100")]
	limit: usize,

	/// The ID after which to start getting playback key pairs
	#[clap(long)]
	after: Option<Ulid>,

	/// The tags to filter playback key pairs by (JSON)
	#[clap(long, default_value = "{}")]
	tags: String,

	/// Reverse the order of the playback key pairs
	#[clap(long)]
	reverse: bool,
}

impl Invokable for Get {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		let resp = invoker
			.invoke(PlaybackKeyPairGetRequest {
				ids: self.ids.iter().copied().map(Into::into).collect(),
				search_options: Some(SearchOptions {
					limit: self.limit as _,
					after_id: self.after.map(Into::into),
					tags: Some(Tags {
						tags: serde_json::from_str(&self.tags).context("failed to parse tags")?,
					}),
					reverse: self.reverse,
				}),
			})
			.await?;

		invoker.display_array(
			&resp
				.playback_key_pairs
				.into_iter()
				.map(PlaybackKeyPair::from_proto)
				.collect::<Vec<_>>(),
		)?;

		Ok(())
	}
}
