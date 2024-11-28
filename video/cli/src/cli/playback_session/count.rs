use anyhow::Context;
use pb::scuffle::video::v1::playback_session_count_request;
use pb::scuffle::video::v1::types::{playback_session_target, PlaybackSessionTarget};
use ulid::Ulid;

use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;

#[derive(Debug, clap::Args)]
pub struct Count {
	/// Count playback sessions by user id
	#[clap(long)]
	user_id: Option<String>,

	/// Count playback sessions by room id
	#[clap(long, conflicts_with = "recording_id", conflicts_with = "user_id")]
	room_id: Option<Ulid>,

	/// Count playback sessions by recording id
	#[clap(long, conflicts_with = "room_id", conflicts_with = "user_id")]
	recording_id: Option<Ulid>,
}

#[derive(Debug, serde::Serialize)]
struct PlaybackSessionCount {
	count: u64,
	deduplicated_count: u64,
}

impl Invokable for Count {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		let resp = invoker
			.invoke(pb::scuffle::video::v1::PlaybackSessionCountRequest {
				filter: match (self.user_id.clone(), self.room_id, self.recording_id) {
					(Some(user_id), None, None) => Some(playback_session_count_request::Filter::UserId(user_id)),
					(None, Some(room_id), None) => {
						Some(playback_session_count_request::Filter::Target(PlaybackSessionTarget {
							target: Some(playback_session_target::Target::RoomId(room_id.into())),
						}))
					}
					(None, None, Some(recording_id)) => {
						Some(playback_session_count_request::Filter::Target(PlaybackSessionTarget {
							target: Some(playback_session_target::Target::RecordingId(recording_id.into())),
						}))
					}
					_ => unreachable!("invalid combination of arguments"),
				},
			})
			.await?;

		invoker
			.display(&PlaybackSessionCount {
				count: resp.count,
				deduplicated_count: resp.deduplicated_count,
			})
			.context("failed to display response")?;

		Ok(())
	}
}
