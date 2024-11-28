use anyhow::Context;
use pb::scuffle::video::v1::RoomModifyRequest;
use ulid::Ulid;

use super::{Room, Visibility};
use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;

#[derive(Debug, clap::Args)]
pub struct Modify {
	/// The id of the room to modify
	#[clap(long, required = true)]
	id: Ulid,

	/// The transcoding config id of the room
	#[clap(long)]
	transcoding_config_id: Option<Ulid>,

	/// The recording config id of the room
	#[clap(long)]
	recording_config_id: Option<Ulid>,

	/// Visibility of the room
	#[clap(long)]
	visibility: Option<Visibility>,

	/// Remove the transcoding config id of the room
	#[clap(long, conflicts_with = "recording_config_id")]
	unset_recording_config_id: bool,

	/// Remove the recording config id of the room
	#[clap(long, conflicts_with = "transcoding_config_id")]
	unset_transcoding_config_id: bool,

	/// The tags for the room (JSON)
	#[clap(long)]
	tags: Option<String>,
}

impl Invokable for Modify {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		let resp = invoker
			.invoke(RoomModifyRequest {
				id: Some(self.id.into()),
				recording_config_id: if self.unset_recording_config_id {
					Some(Ulid::nil().into())
				} else {
					self.recording_config_id.map(Into::into)
				},
				transcoding_config_id: if self.unset_transcoding_config_id {
					Some(Ulid::nil().into())
				} else {
					self.transcoding_config_id.map(Into::into)
				},
				tags: self
					.tags
					.as_ref()
					.map(|tags| {
						anyhow::Ok(pb::scuffle::video::v1::types::Tags {
							tags: serde_json::from_str(tags).context("failed to parse tags")?,
						})
					})
					.transpose()?,
				visibility: self.visibility.map(|v| match v {
					Visibility::Public => pb::scuffle::video::v1::types::Visibility::Public as i32,
					Visibility::Private => pb::scuffle::video::v1::types::Visibility::Private as i32,
				}),
			})
			.await?;

		invoker.display(&Room::from_proto(resp.room.unwrap_or_default(), None))?;

		Ok(())
	}
}
