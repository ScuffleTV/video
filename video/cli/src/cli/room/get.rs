use anyhow::Context;
use pb::scuffle::video::v1::types::{SearchOptions, Tags};
use pb::scuffle::video::v1::RoomGetRequest;
use ulid::Ulid;

use super::{Room, Visibility};
use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;

#[derive(Debug, clap::Args)]
pub struct Get {
	/// The ids of the rooms to get
	#[clap(long, value_parser, num_args = 1.., value_delimiter = ' ')]
	ids: Vec<Ulid>,

	/// Filter by the transcoding config id of the room
	#[clap(long)]
	transcoding_config_id: Option<Ulid>,

	/// Filter by the recording config id of the room
	#[clap(long)]
	recording_config_id: Option<Ulid>,

	/// Filter by the status of the room
	#[clap(long)]
	status: Option<Status>,

	/// Filter by the visibility of the room
	#[clap(long)]
	visibility: Option<Visibility>,

	/// The maximum number of rooms to get
	#[clap(long, default_value = "100")]
	limit: usize,

	/// The ID after which to start getting rooms
	#[clap(long)]
	after: Option<Ulid>,

	/// The tags to filter rooms by (JSON)
	#[clap(long, default_value = "{}")]
	tags: String,

	/// Reverse the order of the rooms
	#[clap(long)]
	reverse: bool,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
	Offline,
	Waiting,
	Ready,
}

impl Invokable for Get {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		let resp = invoker
			.invoke(RoomGetRequest {
				ids: self.ids.iter().copied().map(Into::into).collect(),
				transcoding_config_id: self.transcoding_config_id.map(Into::into),
				recording_config_id: self.recording_config_id.map(Into::into),
				status: self.status.map(|s| match s {
					Status::Offline => pb::scuffle::video::v1::types::RoomStatus::Offline as i32,
					Status::Waiting => pb::scuffle::video::v1::types::RoomStatus::WaitingForTranscoder as i32,
					Status::Ready => pb::scuffle::video::v1::types::RoomStatus::Ready as i32,
				}),
				visibility: self.visibility.map(|v| match v {
					Visibility::Public => pb::scuffle::video::v1::types::Visibility::Public as i32,
					Visibility::Private => pb::scuffle::video::v1::types::Visibility::Private as i32,
				}),
				search_options: Some(SearchOptions {
					limit: self.limit as _,
					after_id: self.after.map(Into::into),
					tags: Some(Tags {
						tags: serde_json::from_str(&self.tags).context("failed to parse tags")?,
					}),
					reverse: self.reverse,
				}),
			})
			.await?;

		invoker.display_array(
			&resp
				.rooms
				.into_iter()
				.map(|room| Room::from_proto(room, None))
				.collect::<Vec<_>>(),
		)?;

		Ok(())
	}
}
