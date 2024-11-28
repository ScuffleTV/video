use anyhow::Context;
use pb::scuffle::video::v1::types::Tags;
use pb::scuffle::video::v1::PlaybackKeyPairTagRequest;
use ulid::Ulid;

use crate::cli::display::TagResponse;
use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;

#[derive(Debug, clap::Args)]
pub struct Tag {
	/// The ids of the playback key pairs to tag
	#[clap(long)]
	id: Ulid,

	/// The tags to add to the playback key pair (JSON)
	#[clap(long, required = true)]
	tags: String,
}

impl Invokable for Tag {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		let resp = invoker
			.invoke(PlaybackKeyPairTagRequest {
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

impl From<(Ulid, pb::scuffle::video::v1::PlaybackKeyPairTagResponse)> for TagResponse {
	fn from((id, resp): (Ulid, pb::scuffle::video::v1::PlaybackKeyPairTagResponse)) -> Self {
		Self {
			id,
			tags: resp.tags.map(|tags| tags.tags).unwrap_or_default(),
		}
	}
}
