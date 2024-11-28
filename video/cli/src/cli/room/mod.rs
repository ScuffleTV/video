use std::collections::HashMap;

use chrono::{TimeZone, Utc};
use pb::ext::UlidExt;
use ulid::Ulid;

use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;
mod create;
mod delete;
mod disconnect;
mod get;
mod modify;
mod reset_key;
mod tag;
mod untag;

#[derive(Debug, clap::Subcommand)]
pub enum Commands {
	/// Get rooms
	Get(get::Get),

	/// Create an room
	Create(create::Create),

	/// Modify room
	Modify(modify::Modify),

	/// Delete rooms
	Delete(delete::Delete),

	/// Disconnect rooms
	Disconnect(disconnect::Disconnect),

	/// Reset stream key for rooms
	ResetKey(reset_key::ResetKey),

	/// Tag rooms
	Tag(tag::Tag),

	/// Untag rooms
	Untag(untag::Untag),
}

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
	Public,
	Private,
}

impl Invokable for Commands {
	async fn invoke(&self, invoker: &mut Invoker, args: &Cli) -> anyhow::Result<()> {
		match self {
			Self::Get(cmd) => cmd.invoke(invoker, args).await,
			Self::Create(cmd) => cmd.invoke(invoker, args).await,
			Self::Modify(cmd) => cmd.invoke(invoker, args).await,
			Self::Delete(cmd) => cmd.invoke(invoker, args).await,
			Self::Disconnect(cmd) => cmd.invoke(invoker, args).await,
			Self::ResetKey(cmd) => cmd.invoke(invoker, args).await,
			Self::Tag(cmd) => cmd.invoke(invoker, args).await,
			Self::Untag(cmd) => cmd.invoke(invoker, args).await,
		}
	}
}

#[derive(Debug, serde::Serialize)]
pub struct Room {
	pub id: Ulid,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub stream_key: Option<String>,
	pub status: String,
	pub visibility: String,
	pub video_input: Option<VideoConfig>,
	pub audio_input: Option<AudioConfig>,
	pub video_output: Vec<VideoConfig>,
	pub audio_output: Vec<AudioConfig>,
	pub active_connection_id: Option<Ulid>,
	pub active_recording_id: Option<Ulid>,
	pub transcoding_config_id: Option<Ulid>,
	pub recording_config_id: Option<Ulid>,
	pub created_at: chrono::DateTime<chrono::Utc>,
	pub updated_at: chrono::DateTime<chrono::Utc>,
	pub last_live_at: Option<chrono::DateTime<chrono::Utc>>,
	pub last_disconnected_at: Option<chrono::DateTime<chrono::Utc>>,
	#[serde(skip_serializing_if = "HashMap::is_empty")]
	pub tags: HashMap<String, String>,
}

impl Room {
	pub fn from_proto(room: pb::scuffle::video::v1::types::Room, stream_key: Option<String>) -> Self {
		Self {
			id: room.id.into_ulid(),
			stream_key,
			visibility: room.visibility().as_str_name().to_string(),
			transcoding_config_id: room.transcoding_config_id.map(|u| u.into_ulid()),
			recording_config_id: room.recording_config_id.map(|u| u.into_ulid()),
			created_at: Utc.timestamp_millis_opt(room.created_at).unwrap(),
			updated_at: Utc.timestamp_millis_opt(room.updated_at).unwrap(),
			last_live_at: room.last_live_at.map(|ts| Utc.timestamp_millis_opt(ts).unwrap()),
			last_disconnected_at: room.last_disconnected_at.map(|ts| Utc.timestamp_millis_opt(ts).unwrap()),
			active_connection_id: room.active_connection_id.map(|u| u.into_ulid()),
			active_recording_id: room.active_recording_id.map(|u| u.into_ulid()),
			status: room.status().as_str_name().to_string(),
			video_input: room.video_input.map(VideoConfig::from_proto),
			audio_input: room.audio_input.map(AudioConfig::from_proto),
			video_output: room.video_output.into_iter().map(VideoConfig::from_proto).collect(),
			audio_output: room.audio_output.into_iter().map(AudioConfig::from_proto).collect(),
			tags: room.tags.map(|tags| tags.tags).unwrap_or_default(),
		}
	}
}

#[derive(Debug, serde::Serialize)]
pub struct VideoConfig {
	pub rendition: String,
	pub bitrate: i64,
	pub fps: i32,
	pub width: i32,
	pub height: i32,
	pub codec: String,
}

impl VideoConfig {
	pub fn from_proto(video_config: pb::scuffle::video::v1::types::VideoConfig) -> Self {
		Self {
			rendition: video_config.rendition().as_str_name().to_string(),
			bitrate: video_config.bitrate,
			fps: video_config.fps,
			width: video_config.width,
			height: video_config.height,
			codec: video_config.codec,
		}
	}
}

#[derive(Debug, serde::Serialize)]
pub struct AudioConfig {
	pub rendition: String,
	pub bitrate: i64,
	pub channels: i32,
	pub sample_rate: i32,
	pub codec: String,
}

impl AudioConfig {
	pub fn from_proto(audio_config: pb::scuffle::video::v1::types::AudioConfig) -> Self {
		Self {
			rendition: audio_config.rendition().as_str_name().to_string(),
			bitrate: audio_config.bitrate,
			channels: audio_config.channels,
			sample_rate: audio_config.sample_rate,
			codec: audio_config.codec,
		}
	}
}
