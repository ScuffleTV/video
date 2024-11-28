use pb::scuffle::video::v1::types::{playback_session_target, PlaybackSessionTarget, SearchOptions};
use pb::scuffle::video::v1::PlaybackSessionGetRequest;
use ulid::Ulid;

use super::PlaybackSession;
use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;

#[derive(Debug, clap::Args)]
pub struct Get {
	/// The ids of the playback sessions to get
	#[clap(long, value_parser, num_args = 1.., value_delimiter = ' ')]
	ids: Vec<Ulid>,

	/// Filter by the user id of the playback session
	#[clap(long)]
	user_id: Option<String>,

	/// Filter by the authentication status of the playback session
	#[clap(long)]
	authorized: Option<bool>,

	/// Filter by the ip address of the playback session
	#[clap(long)]
	ip_address: Option<String>,

	/// Filter by the playback key id of the playback session
	#[clap(long)]
	playback_key_pair_id: Option<Ulid>,

	/// Filter by the room id of the playback session
	#[clap(long)]
	room_id: Option<Ulid>,

	/// Filter by the recording id of the playback session
	#[clap(long, conflicts_with = "room_id")]
	recording_id: Option<Ulid>,

	/// The maximum number of playback sessions to get
	#[clap(long, default_value = "100")]
	limit: usize,

	/// The ID after which to start getting playback sessions
	#[clap(long)]
	after: Option<Ulid>,

	/// Reverse the order of the playback sessions
	#[clap(long)]
	reverse: bool,
}

impl Invokable for Get {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		let resp = invoker
			.invoke(PlaybackSessionGetRequest {
				ids: self.ids.iter().copied().map(Into::into).collect(),
				user_id: self.user_id.clone(),
				authorized: self.authorized,
				ip_address: self.ip_address.clone(),
				playback_key_pair_id: self.playback_key_pair_id.map(Into::into),
				target: match (self.room_id, self.recording_id) {
					(Some(room_id), None) => Some(PlaybackSessionTarget {
						target: Some(playback_session_target::Target::RoomId(room_id.into())),
					}),
					(None, Some(recording_id)) => Some(PlaybackSessionTarget {
						target: Some(playback_session_target::Target::RecordingId(recording_id.into())),
					}),
					(None, None) => None,
					_ => return Err(anyhow::anyhow!("must specify only one of room_id or recording_id")),
				},
				search_options: Some(SearchOptions {
					limit: self.limit as _,
					after_id: self.after.map(Into::into),
					tags: None,
					reverse: self.reverse,
				}),
			})
			.await?;

		invoker.display_array(&resp.sessions.into_iter().map(PlaybackSession::from_proto).collect::<Vec<_>>())?;

		Ok(())
	}
}
