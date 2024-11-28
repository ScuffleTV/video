use anyhow::Context;

use super::{Rendition, TranscodingConfig};
use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;

#[derive(Debug, clap::Args)]
pub struct Create {
	/// Renditions to transcode to
	#[clap(long, value_parser, num_args = 1.., value_delimiter = ' ', required = true)]
	renditions: Vec<Rendition>,

	/// The tags for the transcoding config (JSON)
	#[clap(long, default_value = "{}")]
	tags: String,
}

impl Invokable for Create {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		let resp = invoker
			.invoke(pb::scuffle::video::v1::TranscodingConfigCreateRequest {
				renditions: self.renditions.iter().copied().map(Into::into).collect(),
				tags: Some(pb::scuffle::video::v1::types::Tags {
					tags: serde_json::from_str(&self.tags).context("failed to parse tags")?,
				}),
			})
			.await?;

		invoker.display(&TranscodingConfig::from_proto(resp.transcoding_config.unwrap_or_default()))?;

		Ok(())
	}
}
