use ulid::Ulid;

use super::TranscodingConfig;
use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;

#[derive(Debug, clap::Args)]
pub struct Get {
	/// The ids of the transcoding configs to get
	#[clap(long, value_parser, num_args = 1.., value_delimiter = ' ')]
	ids: Vec<Ulid>,

	/// The maximum number of transcoding configs to get
	#[clap(long, default_value = "100")]
	limit: usize,

	/// The ID after which to start getting transcoding configs
	#[clap(long)]
	after: Option<Ulid>,

	/// The tags to filter transcoding configs by (JSON)
	#[clap(long, default_value = "{}")]
	tags: String,

	/// Reverse the order of the transcoding configs
	#[clap(long)]
	reverse: bool,
}

impl Invokable for Get {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		let resp = invoker
			.invoke(pb::scuffle::video::v1::TranscodingConfigGetRequest {
				ids: self.ids.iter().copied().map(|id| id.into()).collect(),
				search_options: Some(pb::scuffle::video::v1::types::SearchOptions {
					limit: self.limit as _,
					after_id: self.after.map(Into::into),
					tags: Some(pb::scuffle::video::v1::types::Tags {
						tags: serde_json::from_str(&self.tags)?,
					}),
					reverse: self.reverse,
				}),
			})
			.await?;

		invoker.display_array(
			&resp
				.transcoding_configs
				.into_iter()
				.map(TranscodingConfig::from_proto)
				.collect::<Vec<_>>(),
		)?;

		Ok(())
	}
}
