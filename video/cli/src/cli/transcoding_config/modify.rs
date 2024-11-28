use anyhow::Context;
use ulid::Ulid;

use super::{Rendition, TranscodingConfig};
use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;

#[derive(Debug, clap::Args)]
pub struct Modify {
	/// The id of the transcoding config to modify
	#[clap(long, required = true)]
	id: Ulid,

	#[clap(long, value_parser, num_args = 1.., value_delimiter = ' ')]
	/// Renditions to transcode to
	renditions: Option<Vec<Rendition>>,

	/// The tags for the transcoding config (JSON)
	#[clap(long)]
	tags: Option<String>,
}

impl Invokable for Modify {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		if self.renditions.is_none() && self.tags.is_none() {
			anyhow::bail!("at least one flag must be set, --renditions or --tags");
		}

		let resp = invoker
			.invoke(pb::scuffle::video::v1::TranscodingConfigModifyRequest {
				id: Some(self.id.into()),
				renditions: self.renditions.as_ref().map(|r| {
					pb::scuffle::video::v1::transcoding_config_modify_request::RenditionList {
						items: r.iter().copied().map(Into::into).collect(),
					}
				}),
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

		invoker.display(&TranscodingConfig::from_proto(resp.transcoding_config.unwrap_or_default()))?;

		Ok(())
	}
}
