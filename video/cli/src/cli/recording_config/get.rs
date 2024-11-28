use anyhow::Context;
use ulid::Ulid;

use super::RecordingConfig;
use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;

#[derive(Debug, clap::Args)]
pub struct Get {
	/// The ids of the recording configs to get
	#[clap(long = "id")]
	ids: Vec<Ulid>,

	/// The maximum number of recording configs to get
	#[clap(long, default_value = "100")]
	limit: usize,

	/// The ID after which to start getting recording configs
	#[clap(long)]
	after: Option<Ulid>,

	/// The tags to filter recording configs by (JSON)
	#[clap(long, default_value = "{}")]
	tags: String,

	/// Reverse the order of the recording configs
	#[clap(long)]
	reverse: bool,
}

impl Invokable for Get {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		let resp = invoker
			.invoke(pb::scuffle::video::v1::RecordingConfigGetRequest {
				ids: self.ids.iter().copied().map(Into::into).collect(),
				search_options: Some(pb::scuffle::video::v1::types::SearchOptions {
					limit: self.limit as _,
					after_id: self.after.map(Into::into),
					tags: Some(pb::scuffle::video::v1::types::Tags {
						tags: serde_json::from_str(&self.tags).context("failed to parse tags")?,
					}),
					reverse: self.reverse,
				}),
			})
			.await?;

		invoker.display_array(
			&resp
				.recording_configs
				.into_iter()
				.map(RecordingConfig::from_proto)
				.collect::<Vec<_>>(),
		)?;

		Ok(())
	}
}
