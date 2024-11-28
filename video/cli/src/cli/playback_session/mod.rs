use chrono::{TimeZone, Utc};
use pb::ext::UlidExt;
use pb::scuffle::video::v1::types::playback_session_target;
use ulid::Ulid;

use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;

mod count;
mod get;
mod revoke;

#[derive(Debug, clap::Subcommand)]
pub enum Commands {
	/// Get playback sessions
	Get(get::Get),

	/// Revoke playback sessions
	Revoke(revoke::Revoke),

	/// Count playback sessions
	Count(count::Count),
}

impl Invokable for Commands {
	async fn invoke(&self, invoker: &mut Invoker, args: &Cli) -> anyhow::Result<()> {
		match self {
			Self::Get(cmd) => cmd.invoke(invoker, args).await,
			Self::Revoke(cmd) => cmd.invoke(invoker, args).await,
			Self::Count(cmd) => cmd.invoke(invoker, args).await,
		}
	}
}

#[derive(Debug, serde::Serialize)]
pub struct PlaybackSession {
	pub id: Ulid,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub room_id: Option<Ulid>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub recording_id: Option<Ulid>,
	pub user_id: Option<String>,
	pub playback_key_pair_id: Option<Ulid>,
	pub issued_at: Option<chrono::DateTime<chrono::Utc>>,
	pub created_at: chrono::DateTime<chrono::Utc>,
	pub last_active_at: chrono::DateTime<chrono::Utc>,
	pub ip_address: String,
	pub user_agent: Option<String>,
	pub referer: Option<String>,
	pub origin: Option<String>,
	pub device: String,
	pub platform: String,
	pub browser: String,
	pub player_version: Option<String>,
}

impl PlaybackSession {
	fn from_proto(pb: pb::scuffle::video::v1::types::PlaybackSession) -> Self {
		PlaybackSession {
			browser: pb.browser().as_str_name().to_owned(),
			device: pb.device().as_str_name().to_owned(),
			platform: pb.platform().as_str_name().to_owned(),
			id: pb.id.into_ulid(),
			user_id: pb.user_id,
			ip_address: pb.ip_address,
			playback_key_pair_id: pb.playback_key_pair_id.map(|s| s.into_ulid()),
			room_id: pb.target.and_then(|target| match target.target {
				Some(playback_session_target::Target::RoomId(room_id)) => Some(room_id.into_ulid()),
				_ => None,
			}),
			recording_id: pb.target.and_then(|target| match target.target {
				Some(playback_session_target::Target::RecordingId(recording_id)) => Some(recording_id.into_ulid()),
				_ => None,
			}),
			created_at: Utc.timestamp_millis_opt(pb.created_at).unwrap(),
			issued_at: pb.issued_at.map(|ts| Utc.timestamp_millis_opt(ts).unwrap()),
			origin: pb.origin,
			referer: pb.referer,
			user_agent: pb.user_agent,
			player_version: pb.player_version,
			last_active_at: Utc.timestamp_millis_opt(pb.last_active_at).unwrap(),
		}
	}
}
