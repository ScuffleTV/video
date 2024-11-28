use anyhow::Context;
use pb::scuffle::video::v1::RoomCreateRequest;
use ulid::Ulid;

use super::{Room, Visibility};
use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;

#[derive(Debug, clap::Args)]
pub struct Create {
	/// The transcoding config id of the room
	#[clap(long)]
	transcoding_config_id: Option<Ulid>,

	/// The recording config id of the room
	#[clap(long)]
	recording_config_id: Option<Ulid>,

	/// Visibility of the room
	#[clap(long, default_value = "public")]
	visibility: Visibility,

	/// The tags for the room (JSON)
	#[clap(long, default_value = "{}")]
	tags: String,
}

impl Invokable for Create {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		let resp = invoker
			.invoke(RoomCreateRequest {
				transcoding_config_id: self.transcoding_config_id.map(Into::into),
				recording_config_id: self.recording_config_id.map(Into::into),
				visibility: match self.visibility {
					Visibility::Public => pb::scuffle::video::v1::types::Visibility::Public as i32,
					Visibility::Private => pb::scuffle::video::v1::types::Visibility::Private as i32,
				},
				tags: Some(pb::scuffle::video::v1::types::Tags {
					tags: serde_json::from_str(&self.tags).context("failed to parse tags")?,
				}),
			})
			.await?;

		invoker.display(&Room::from_proto(resp.room.unwrap_or_default(), Some(resp.stream_key)))?;

		Ok(())
	}
}
