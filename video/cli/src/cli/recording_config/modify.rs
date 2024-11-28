use anyhow::Context;
use pb::scuffle::video::v1::recording_config_modify_request::{LifecyclePolicyList, RenditionList};
use ulid::Ulid;

use super::{LifecyclePolicy, RecordingConfig, Rendition};
use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;

#[derive(Debug, clap::Args)]
pub struct Modify {
	/// The id of the recording config to modify
	#[clap(long, required = true)]
	id: Ulid,

	#[clap(long)]
	/// Renditions to save for the recording
	renditions: Option<Vec<Rendition>>,

	#[clap(long)]
	/// The s3 bucket id to save the recording to
	s3_bucket_id: Option<Ulid>,

	#[clap(long)]
	/// The lifecycle policies to apply to the recording (JSON)
	lifecycle_policies: Option<Vec<String>>,

	/// The tags for the recording config (JSON)
	#[clap(long)]
	tags: Option<String>,
}

impl Invokable for Modify {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		if self.renditions.is_none()
			&& self.s3_bucket_id.is_none()
			&& self.lifecycle_policies.is_none()
			&& self.tags.is_none()
		{
			anyhow::bail!("at least one flag must be set, --renditions, --s3-bucket-id, --lifecycle-policies, or --tags");
		}

		let resp = invoker
			.invoke(pb::scuffle::video::v1::RecordingConfigModifyRequest {
				id: Some(self.id.into()),
				stored_renditions: self.renditions.as_ref().map(|r| RenditionList {
					items: r.iter().copied().map(Into::into).collect(),
				}),
				s3_bucket_id: self.s3_bucket_id.map(Into::into),
				lifecycle_policies: self
					.lifecycle_policies
					.as_ref()
					.map(|p| {
						anyhow::Ok(LifecyclePolicyList {
							items: p
								.iter()
								.map(|p| {
									serde_json::from_str::<LifecyclePolicy>(p)
										.context("failed to parse lifecycle policy")
										.and_then(TryInto::try_into)
								})
								.collect::<anyhow::Result<Vec<_>>>()?,
						})
					})
					.transpose()?,
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

		invoker.display(&RecordingConfig::from_proto(resp.recording_config.unwrap_or_default()))?;

		Ok(())
	}
}
