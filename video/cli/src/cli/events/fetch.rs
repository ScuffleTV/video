use anyhow::Context;
use chrono::TimeZone;
use futures_util::StreamExt;
use pb::ext::UlidExt;
use pb::scuffle::video::v1::types::event;
use pb::scuffle::video::v1::types::event::recording_config;
use pb::scuffle::video::v1::{events_fetch_request, EventsFetchRequest};
use ulid::Ulid;

use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;

#[derive(Debug, clap::Args)]
pub struct Fetch {
	/// The target to fetch events for
	#[clap(long, required = true)]
	target: Target,

	/// The maximum number of events to fetch
	#[clap(long, default_value = "100")]
	limit: usize,

	/// The max1imum delay to wait for events in milliseconds
	#[clap(long, default_value = "60000")]
	max_delay: u32,

	/// Whether to fetch events once or continuously
	#[clap(long)]
	once: bool,

	/// Disable acknowledging events
	#[clap(long)]
	no_ack: bool,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
	Room,
	Recording,
	AccessToken,
	PlaybackKeyPair,
	RecordingConfig,
	TranscodingConfig,
	S3Bucket,
}

#[derive(Debug, serde::Serialize)]
struct Event {
	id: Ulid,
	timestamp: chrono::DateTime<chrono::Utc>,
	payload: EventPayload,
}

#[derive(Debug, Default, serde::Serialize)]
struct EventPayload {
	resource_id: Ulid,
	resource: String,
	action: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	error: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	recording_config_id: Option<Ulid>,
	#[serde(skip_serializing_if = "Option::is_none")]
	room_id: Option<Ulid>,
	#[serde(skip_serializing_if = "Option::is_none")]
	connection_id: Option<Ulid>,
	#[serde(skip_serializing_if = "Option::is_none")]
	clean: Option<bool>,
	#[serde(skip_serializing_if = "Option::is_none")]
	cause: Option<String>,
}

impl Invokable for Fetch {
	async fn invoke(&self, invoker: &mut Invoker, cli: &Cli) -> anyhow::Result<()> {
		loop {
			let mut resp = invoker
				.invoke(EventsFetchRequest {
					target: match self.target {
						Target::Room => events_fetch_request::Target::Room.into(),
						Target::Recording => events_fetch_request::Target::Recording.into(),
						Target::AccessToken => events_fetch_request::Target::AccessToken.into(),
						Target::PlaybackKeyPair => events_fetch_request::Target::PlaybackKeyPair.into(),
						Target::RecordingConfig => events_fetch_request::Target::RecordingConfig.into(),
						Target::TranscodingConfig => events_fetch_request::Target::TranscodingConfig.into(),
						Target::S3Bucket => events_fetch_request::Target::S3Bucket.into(),
					},
					max_events: self.limit as _,
					max_delay_ms: self.max_delay,
				})
				.await?;

			while let Some(event) = resp.next().await {
				let event = event
					.context("failed to get event")?
					.event
					.ok_or_else(|| anyhow::anyhow!("event missing"))?;

				if !self.no_ack {
					invoker
						.invoke(pb::scuffle::video::v1::EventsAckRequest {
							id: event.event_id,
							action: Some(pb::scuffle::video::v1::events_ack_request::Action::Ack(true)),
						})
						.await?;
				}

				let event = Event {
					id: event.event_id.into_ulid(),
					timestamp: chrono::Utc.timestamp_millis_opt(event.timestamp).unwrap(),
					payload: match event.event {
						Some(event::Event::Recording(recording)) => match recording.event {
							Some(event::recording::Event::Started(started)) => EventPayload {
								resource_id: recording.recording_id.into_ulid(),
								resource: "recording".to_owned(),
								action: "started".to_owned(),
								recording_config_id: Some(started.recording_config_id.into_ulid()),
								room_id: Some(started.room_id.into_ulid()),
								..Default::default()
							},
							Some(event::recording::Event::Finished(_)) => EventPayload {
								resource_id: recording.recording_id.into_ulid(),
								resource: "recording".to_owned(),
								action: "finished".to_owned(),
								..Default::default()
							},
							Some(event::recording::Event::Modified(_)) => EventPayload {
								resource_id: recording.recording_id.into_ulid(),
								resource: "recording".to_owned(),
								action: "modified".to_owned(),
								..Default::default()
							},
							Some(event::recording::Event::Deleted(deleted)) => match deleted.event {
								Some(event::recording::deleted::Event::Started(deleted_started)) => EventPayload {
									resource_id: recording.recording_id.into_ulid(),
									resource: "recording".to_owned(),
									action: "deleted:started".to_owned(),
									recording_config_id: deleted_started.recording_config_id.map(|id| id.into_ulid()),
									..Default::default()
								},
								Some(event::recording::deleted::Event::Failed(failed)) => EventPayload {
									resource_id: recording.recording_id.into_ulid(),
									resource: "recording".to_owned(),
									action: "deleted:failed".to_owned(),
									error: Some(failed.error),
									..Default::default()
								},
								Some(event::recording::deleted::Event::Finished(_)) => EventPayload {
									resource_id: recording.recording_id.into_ulid(),
									resource: "recording".to_owned(),
									action: "deleted:finished".to_owned(),
									error: None,
									..Default::default()
								},
								None => return Err(anyhow::anyhow!("recording deleted event missing")),
							},
							Some(event::recording::Event::Failed(failed)) => EventPayload {
								resource_id: recording.recording_id.into_ulid(),
								resource: "recording".to_owned(),
								action: "deletedfailed".to_owned(),
								error: Some(failed.error),
								..Default::default()
							},
							None => return Err(anyhow::anyhow!("recording event missing")),
						},
						Some(event::Event::RecordingConfig(recording_config)) => match recording_config.event {
							Some(recording_config::Event::Created(_)) => EventPayload {
								resource_id: recording_config.recording_config_id.into_ulid(),
								resource: "recording_config".to_owned(),
								action: "created".to_owned(),
								..Default::default()
							},
							Some(recording_config::Event::Modified(_)) => EventPayload {
								resource_id: recording_config.recording_config_id.into_ulid(),
								resource: "recording_config".to_owned(),
								action: "modified".to_owned(),
								..Default::default()
							},
							Some(recording_config::Event::Deleted(_)) => EventPayload {
								resource_id: recording_config.recording_config_id.into_ulid(),
								resource: "recording_config".to_owned(),
								action: "deleted".to_owned(),
								..Default::default()
							},
							None => return Err(anyhow::anyhow!("recording config event missing")),
						},
						Some(event::Event::TranscodingConfig(transcoding_config)) => match transcoding_config.event {
							Some(event::transcoding_config::Event::Created(_)) => EventPayload {
								resource_id: transcoding_config.transcoding_config_id.into_ulid(),
								resource: "transcoding_config".to_owned(),
								action: "created".to_owned(),
								..Default::default()
							},
							Some(event::transcoding_config::Event::Modified(_)) => EventPayload {
								resource_id: transcoding_config.transcoding_config_id.into_ulid(),
								resource: "transcoding_config".to_owned(),
								action: "modified".to_owned(),
								..Default::default()
							},
							Some(event::transcoding_config::Event::Deleted(_)) => EventPayload {
								resource_id: transcoding_config.transcoding_config_id.into_ulid(),
								resource: "transcoding_config".to_owned(),
								action: "deleted".to_owned(),
								..Default::default()
							},
							None => return Err(anyhow::anyhow!("transcoding config event missing")),
						},
						Some(event::Event::S3Bucket(s3_bucket)) => match s3_bucket.event {
							Some(event::s3_bucket::Event::Created(_)) => EventPayload {
								resource_id: s3_bucket.s3_bucket_id.into_ulid(),
								resource: "s3_bucket".to_owned(),
								action: "created".to_owned(),
								..Default::default()
							},
							Some(event::s3_bucket::Event::Modified(_)) => EventPayload {
								resource_id: s3_bucket.s3_bucket_id.into_ulid(),
								resource: "s3_bucket".to_owned(),
								action: "modified".to_owned(),
								..Default::default()
							},
							Some(event::s3_bucket::Event::Deleted(_)) => EventPayload {
								resource_id: s3_bucket.s3_bucket_id.into_ulid(),
								resource: "s3_bucket".to_owned(),
								action: "deleted".to_owned(),
								..Default::default()
							},
							None => return Err(anyhow::anyhow!("s3 bucket event missing")),
						},
						Some(event::Event::AccessToken(access_token)) => match access_token.event {
							Some(event::access_token::Event::Created(_)) => EventPayload {
								resource_id: access_token.access_token_id.into_ulid(),
								resource: "access_token".to_owned(),
								action: "created".to_owned(),
								..Default::default()
							},
							Some(event::access_token::Event::Modified(_)) => EventPayload {
								resource_id: access_token.access_token_id.into_ulid(),
								resource: "access_token".to_owned(),
								action: "modified".to_owned(),
								..Default::default()
							},
							Some(event::access_token::Event::Deleted(_)) => EventPayload {
								resource_id: access_token.access_token_id.into_ulid(),
								resource: "access_token".to_owned(),
								action: "deleted".to_owned(),
								..Default::default()
							},
							None => return Err(anyhow::anyhow!("access token event missing")),
						},
						Some(event::Event::PlaybackKeyPair(playback_key_pair)) => match playback_key_pair.event {
							Some(event::playback_key_pair::Event::Created(_)) => EventPayload {
								resource_id: playback_key_pair.playback_key_pair_id.into_ulid(),
								resource: "playback_key_pair".to_owned(),
								action: "created".to_owned(),
								..Default::default()
							},
							Some(event::playback_key_pair::Event::Modified(_)) => EventPayload {
								resource_id: playback_key_pair.playback_key_pair_id.into_ulid(),
								resource: "playback_key_pair".to_owned(),
								action: "modified".to_owned(),
								..Default::default()
							},
							Some(event::playback_key_pair::Event::Deleted(_)) => EventPayload {
								resource_id: playback_key_pair.playback_key_pair_id.into_ulid(),
								resource: "playback_key_pair".to_owned(),
								action: "deleted".to_owned(),
								..Default::default()
							},
							None => return Err(anyhow::anyhow!("playback key pair event missing")),
						},
						Some(event::Event::Room(room)) => match room.event {
							Some(event::room::Event::Created(_)) => EventPayload {
								resource_id: room.room_id.into_ulid(),
								resource: "room".to_owned(),
								action: "created".to_owned(),
								..Default::default()
							},
							Some(event::room::Event::Connected(connected)) => EventPayload {
								resource_id: room.room_id.into_ulid(),
								resource: "room".to_owned(),
								action: "connected".to_owned(),
								connection_id: Some(connected.connection_id.into_ulid()),
								..Default::default()
							},
							Some(event::room::Event::Disconnected(disconnected)) => EventPayload {
								resource_id: room.room_id.into_ulid(),
								resource: "room".to_owned(),
								action: "disconnected".to_owned(),
								connection_id: Some(disconnected.connection_id.into_ulid()),
								clean: Some(disconnected.clean),
								cause: disconnected.cause,
								..Default::default()
							},
							Some(event::room::Event::TranscoderDisconnected(disconnected)) => EventPayload {
								resource_id: room.room_id.into_ulid(),
								resource: "room".to_owned(),
								action: "transcoder_disconnected".to_owned(),
								connection_id: Some(disconnected.connection_id.into_ulid()),
								clean: Some(disconnected.clean),
								..Default::default()
							},
							Some(event::room::Event::Ready(ready)) => EventPayload {
								resource_id: room.room_id.into_ulid(),
								resource: "room".to_owned(),
								action: "ready".to_owned(),
								connection_id: Some(ready.connection_id.into_ulid()),
								..Default::default()
							},
							Some(event::room::Event::Modified(_)) => EventPayload {
								resource_id: room.room_id.into_ulid(),
								resource: "room".to_owned(),
								action: "modified".to_owned(),
								..Default::default()
							},
							Some(event::room::Event::Deleted(_)) => EventPayload {
								resource_id: room.room_id.into_ulid(),
								resource: "room".to_owned(),
								action: "deleted".to_owned(),
								..Default::default()
							},
							Some(event::room::Event::Failed(failed)) => EventPayload {
								resource_id: room.room_id.into_ulid(),
								resource: "room".to_owned(),
								action: "failed".to_owned(),
								error: Some(failed.error),
								..Default::default()
							},
							None => return Err(anyhow::anyhow!("room event missing")),
						},
						None => return Err(anyhow::anyhow!("event missing")),
					},
				};

				if cli.json {
					invoker.display(&event)?;
				} else {
					invoker.display_array(&[event])?;
				}

				println!();
			}

			if self.once {
				break;
			}
		}

		Ok(())
	}
}
