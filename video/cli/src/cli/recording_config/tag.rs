use anyhow::Context;
use pb::scuffle::video::v1::types::Tags;
use pb::scuffle::video::v1::RecordingConfigTagRequest;
use ulid::Ulid;

use crate::cli::display::TagResponse;
use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;

#[derive(Debug, clap::Args)]
pub struct Tag {
	/// The ids of the recording configs to tag
	#[clap(long, required = true)]
	id: Ulid,

	/// The tags to add to the recording config (JSON)
	#[clap(long, required = true)]
	tags: String,
}

impl Invokable for Tag {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		let resp = invoker
			.invoke(RecordingConfigTagRequest {
				id: Some(self.id.into()),
				tags: Some(Tags {
					tags: serde_json::from_str(&self.tags).context("failed to parse tags")?,
				}),
			})
			.await?;

		invoker.display(&TagResponse::from((self.id, resp)))?;

		Ok(())
	}
}

impl From<(Ulid, pb::scuffle::video::v1::RecordingConfigTagResponse)> for TagResponse {
	fn from((id, resp): (Ulid, pb::scuffle::video::v1::RecordingConfigTagResponse)) -> Self {
		Self {
			id,
			tags: resp.tags.map(|tags| tags.tags).unwrap_or_default(),
		}
	}
}
