use anyhow::Context;
use pb::scuffle::video::v1::RecordingModifyRequest;
use ulid::Ulid;

use super::{Recording, Visibility};
use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;

#[derive(Debug, clap::Args)]
pub struct Modify {
	/// The id of the recording to modify
	#[clap(long, required = true)]
	id: Ulid,

	/// The room id of the recording
	#[clap(long)]
	room_id: Option<Ulid>,

	/// The recording config id of the recording
	#[clap(long)]
	recording_config_id: Option<Ulid>,

	/// The visibility of the recording
	#[clap(long)]
	visibility: Option<Visibility>,

	/// The tags for the recording (JSON)
	#[clap(long)]
	tags: Option<String>,
}

impl Invokable for Modify {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		if self.room_id.is_none() && self.recording_config_id.is_none() && self.visibility.is_none() && self.tags.is_none() {
			anyhow::bail!("at least one of --room-id, --recording-config-id, --visibility, or --tags must be specified");
		}

		let resp = invoker
			.invoke(RecordingModifyRequest {
				id: Some(self.id.into()),
				room_id: self.room_id.map(Into::into),
				recording_config_id: self.recording_config_id.map(Into::into),
				visibility: self.visibility.map(|v| match v {
					Visibility::Public => pb::scuffle::video::v1::types::Visibility::Public as i32,
					Visibility::Private => pb::scuffle::video::v1::types::Visibility::Private as i32,
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

		invoker
			.display(&Recording::from_proto(resp.recording.unwrap_or_default()))
			.context("failed to display response")?;

		Ok(())
	}
}
