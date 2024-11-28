use anyhow::Context;
use ulid::Ulid;

use super::{LifecyclePolicy, RecordingConfig, Rendition};
use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;

#[derive(Debug, clap::Args)]
pub struct Create {
	/// Renditions to save for the recording
	#[clap(long, required = true, value_delimiter = ' ', value_parser, num_args = 1..)]
	renditions: Vec<Rendition>,

	/// The s3 bucket id to save the recording to
	#[clap(long)]
	s3_bucket_id: Option<Ulid>,

	/// The lifecycle policies to apply to the recording (JSON)
	#[clap(long)]
	lifecycle_policies: Vec<String>,

	/// The tags for the recording config (JSON)
	#[clap(long, default_value = "{}")]
	tags: String,
}

impl Invokable for Create {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		let resp = invoker
			.invoke(pb::scuffle::video::v1::RecordingConfigCreateRequest {
				stored_renditions: self.renditions.iter().copied().map(Into::into).collect(),
				s3_bucket_id: self.s3_bucket_id.map(Into::into),
				lifecycle_policies: self
					.lifecycle_policies
					.iter()
					.map(|p| {
						serde_json::from_str::<LifecyclePolicy>(p)
							.context("failed to parse lifecycle policy")
							.and_then(TryInto::try_into)
					})
					.collect::<anyhow::Result<Vec<_>>>()?,
				tags: Some(pb::scuffle::video::v1::types::Tags {
					tags: serde_json::from_str(&self.tags).context("failed to parse tags")?,
				}),
			})
			.await?;

		invoker.display(&RecordingConfig::from_proto(resp.recording_config.unwrap_or_default()))?;

		Ok(())
	}
}
