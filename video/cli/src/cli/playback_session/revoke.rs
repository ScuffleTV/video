use anyhow::Context;
use pb::scuffle::video::v1::types::{playback_session_target, PlaybackSessionTarget};
use pb::scuffle::video::v1::PlaybackSessionRevokeRequest;
use ulid::Ulid;

use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;

#[derive(Debug, clap::Args)]
pub struct Revoke {
	/// The ids of the playback sessions to revoke
	#[clap(long, value_parser, num_args = 1.., value_delimiter = ' ')]
	ids: Vec<Ulid>,

	/// The user id of the playback sessions to revoke
	#[clap(long)]
	user_id: Option<String>,

	/// The room id of the playback sessions to revoke
	#[clap(long)]
	room_id: Option<Ulid>,

	/// The recording id of the playback sessions to revoke
	#[clap(long, conflicts_with = "room_id")]
	recording_id: Option<Ulid>,

	/// Authentication status of the playback sessions to revoke
	#[clap(long)]
	authorized: Option<bool>,

	/// The time before which to revoke playback sessions
	#[clap(long)]
	before: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, serde::Serialize)]
struct PlaybackSessionRevoke {
	count: usize,
}

impl Invokable for Revoke {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		let result = invoker
			.invoke(PlaybackSessionRevokeRequest {
				ids: self.ids.iter().copied().map(Into::into).collect(),
				user_id: self.user_id.clone(),
				target: if let Some(room_id) = self.room_id {
					Some(PlaybackSessionTarget {
						target: Some(playback_session_target::Target::RoomId(room_id.into())),
					})
				} else {
					self.recording_id.map(|recording_id| PlaybackSessionTarget {
						target: Some(playback_session_target::Target::RecordingId(recording_id.into())),
					})
				},
				authorized: self.authorized,
				before: self.before.map(|before| before.timestamp_millis()),
			})
			.await?;

		invoker
			.display(&PlaybackSessionRevoke {
				count: result.revoked as _,
			})
			.context("failed to display response")?;

		Ok(())
	}
}
