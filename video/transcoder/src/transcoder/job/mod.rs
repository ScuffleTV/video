use std::collections::{HashMap, HashSet};
use std::pin::pin;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use async_nats::jetstream::Message;
use bytes::Bytes;
use futures::{FutureExt, StreamExt};
use futures_util::TryFutureExt;
use pb::ext::UlidExt;
use pb::scuffle::video::internal::events::TranscoderRequestTask;
use pb::scuffle::video::internal::ingest_client::IngestClient;
use pb::scuffle::video::internal::live_rendition_manifest::RenditionInfo;
use pb::scuffle::video::internal::{
	ingest_watch_request, ingest_watch_response, IngestWatchRequest, IngestWatchResponse, LiveManifest,
	LiveRenditionManifest,
};
use pb::scuffle::video::v1::events_fetch_request::Target;
use pb::scuffle::video::v1::types::event;
use prost::Message as _;
use scuffle_utils::prelude::FutureTimeout;
use scuffle_utils::task::AsyncTask;
use tokio::sync::mpsc;
use tokio::{select, try_join};
use tokio_util::sync::CancellationToken;
use ulid::Ulid;
use video_common::database::Rendition;

use self::recording::Recording;
use self::task::generic::GenericTask;
use self::track::parser::TrackOut;
use self::track::Track;
use crate::global::TranscoderGlobal;
use crate::transcoder::job::ffmpeg::Transcoder;
use crate::transcoder::job::sql_operations::perform_sql_operations;
use crate::transcoder::job::task::generic::generic_task;
use crate::transcoder::job::task::rendition::track_task;
use crate::transcoder::job::task::track_parser::track_parser_task;
use crate::transcoder::job::track::parser::TrackParser;

mod breakpoint;
mod ffmpeg;
mod recording;
mod renditions;
mod screenshot;
mod sql_operations;
mod task;
mod track;

pub async fn handle_message<G: TranscoderGlobal>(global: Arc<G>, msg: Message, shutdown_token: CancellationToken) {
	let mut job = match Job::new(&global, &msg).await {
		Ok(job) => job,
		Err(err) => {
			msg.ack_with(async_nats::jetstream::AckKind::Nak(Some(Duration::from_secs(15))))
				.await
				.ok();
			tracing::error!(error = %err, "failed to handle message");
			return;
		}
	};

	if let Err(err) = msg.double_ack().await {
		tracing::error!(error = %err, "failed to ACK message");
		return;
	};

	if let Err(err) = job.run(&global, shutdown_token).await {
		video_common::events::emit(
			global.nats(),
			&global.config().events_stream_name,
			job.organization_id,
			Target::Room,
			event::Event::Room(event::Room {
				room_id: Some(job.room_id.into()),
				event: Some(event::room::Event::Failed(event::room::Failed {
					connection_id: Some(job.connection_id.into()),
					error: err.to_string(),
				})),
			}),
		)
		.await;

		println!("failed to run transcoder: {:#}", err);

		tracing::error!(error = %err, "failed to run transcoder");
	}

	if let Err(err) = job.handle_shutdown().await {
		tracing::error!(error = %err, "failed to shutdown transcoder");
	}

	tracing::info!("stream finished");
}

struct Job {
	organization_id: Ulid,
	room_id: Ulid,
	connection_id: Ulid,

	recording: Option<Recording>,

	ingest_ready: bool,
	transcoder_ready: bool,

	tracks: HashMap<Rendition, Track>,
	generic_uploader: mpsc::Sender<GenericTask>,

	ffmpeg_send: Option<mpsc::Sender<Bytes>>,
	ffmpeg_recv: mpsc::Receiver<(Rendition, TrackOut)>,

	screenshot_recv: mpsc::Receiver<(Bytes, f64)>,

	tasks: Vec<AsyncTask<anyhow::Result<()>>>,

	first_init_put: bool,
	screenshot_idx: u32,

	ingest_send: mpsc::Sender<IngestWatchRequest>,
	ingest_recv: tonic::Streaming<IngestWatchResponse>,
	ingest_shutdown: Option<ingest_watch_response::Shutdown>,
}

impl Job {
	async fn new(global: &Arc<impl TranscoderGlobal>, msg: &Message) -> Result<Self> {
		let message = TranscoderRequestTask::decode(msg.payload.clone())?;

		let organization_id = message.organization_id.into_ulid();
		let room_id = message.room_id.into_ulid();
		let connection_id = message.connection_id.into_ulid();

		let result = perform_sql_operations(global, organization_id, room_id, connection_id).await?;

		tracing::info!(
			%organization_id,
			%room_id,
			%connection_id,
			transcoding_config_id = %result.transcoding_config.id.into_ulid(),
			recording_id = %result.recording.as_ref().map(|r| r.id().to_string()).unwrap_or_default(),
			"got new stream request",
		);

		let renditions = result
			.video_output
			.iter()
			.map(|r| r.rendition())
			.chain(result.audio_output.iter().map(|r| r.rendition()))
			.map(Into::into)
			.collect::<HashSet<Rendition>>();

		let (ffmpeg_input, input_receiver) = mpsc::channel(1);
		let (track_parser, ffmpeg_output) = mpsc::channel(renditions.len());

		let mut recording = result.recording;
		let mut ffmpeg_outputs = HashMap::new();

		let mut tasks = recording.as_mut().map(|r| r.tasks()).unwrap_or_default();

		tasks.extend(renditions.iter().copied().map(|rendition| {
			let (tx, rx) = mpsc::channel(1);
			ffmpeg_outputs.insert(rendition, tx);

			let tp = TrackParser::new(rx);
			AsyncTask::spawn(
				format!("track_parser({rendition})"),
				track_parser_task(tp, rendition, track_parser.clone()),
			)
		}));

		let mut tracks = HashMap::new();

		tasks.extend(renditions.iter().copied().map(|rendition| {
			let (tx, rx) = mpsc::channel(16);
			tracks.insert(rendition, Track::new(global, rendition, tx));

			AsyncTask::spawn(
				format!("rendition({rendition})"),
				track_task(global.clone(), organization_id, room_id, connection_id, rendition, rx),
			)
		}));

		let (frame_send, frame_recv) = mpsc::channel(1);
		tasks.push(AsyncTask::spawn_blocking("ffmpeg", {
			let global = global.clone();

			let video_configs = result.video_output.clone();
			let audio_configs = result.audio_output.clone();

			move || {
				Transcoder::new(
					&global,
					input_receiver,
					frame_send,
					ffmpeg_outputs,
					video_configs,
					audio_configs,
				)?
				.run()
			}
		}));

		let (screenshot_send, screenshot_recv) = mpsc::channel(16);
		tasks.push(AsyncTask::spawn_blocking("screenshot", || {
			screenshot::screenshot_task(frame_recv, screenshot_send)
		}));

		let (generic_uploader, rx) = mpsc::channel(16);

		tasks.push(AsyncTask::spawn(
			"generic",
			generic_task(global.clone(), organization_id, room_id, connection_id, rx),
		));

		tracing::debug!(endpoint = %message.grpc_endpoint, "trying to connect to ingest");

		let tls = global.ingest_tls();

		let channel = scuffle_utilsgrpc::make_channel(vec![message.grpc_endpoint], Duration::from_secs(30), tls)?;

		let mut client = IngestClient::new(channel);

		let (ingest_send, rx) = mpsc::channel(16);

		ingest_send
			.send(IngestWatchRequest {
				message: Some(ingest_watch_request::Message::Open(ingest_watch_request::Open {
					request_id: message.request_id,
				})),
			})
			.await
			.context("failed to send open message")?;

		let ingest_recv = client
			.watch(tokio_stream::wrappers::ReceiverStream::new(rx))
			.timeout(Duration::from_secs(2))
			.await
			.context("failed to connect to ingest")??
			.into_inner();

		Ok(Self {
			organization_id,
			room_id,
			connection_id,
			recording,
			screenshot_idx: 0,
			ingest_ready: false,
			transcoder_ready: false,
			tracks,
			ingest_send,
			ingest_recv,
			first_init_put: true,
			ffmpeg_send: Some(ffmpeg_input),
			tasks,
			ingest_shutdown: None,
			ffmpeg_recv: ffmpeg_output,
			generic_uploader,
			screenshot_recv,
		})
	}

	async fn run(&mut self, global: &Arc<impl TranscoderGlobal>, shutdown_token: CancellationToken) -> Result<()> {
		let mut shutdown_fuse = pin!(shutdown_token.cancelled().fuse());

		let mut upload_init_timer = tokio::time::interval(Duration::from_secs(15));

		while self.ffmpeg_send.is_some() {
			select! {
				_ = &mut shutdown_fuse => {
					self.ingest_send.try_send(IngestWatchRequest {
						message: Some(ingest_watch_request::Message::Shutdown(
							ingest_watch_request::Shutdown::Request as i32,
						))
					})?;
				},
				_ = upload_init_timer.tick() => {
					self.update_manifest()?;
				}
				r = self.ffmpeg_recv.recv() => {
					let Some((rendition, track_out)) = r else {
						break;
					};

					self.handle_track(rendition, track_out)?;
				},
				Some((data, time)) = self.screenshot_recv.recv() => {
					self.screenshot_idx += 1;

					if let Some(recording) = &mut self.recording {
						recording.upload_thumbnail(self.screenshot_idx, time, data.clone())?;
					}

					self.generic_uploader
						.try_send(GenericTask::Screenshot { data, idx: self.screenshot_idx })
						.context("send screenshot task")?;

					self.update_manifest()?;
					self.ready()?;
				},
				msg = self.ingest_recv.next() => {
					let Some(msg) = msg else {
						if self.ingest_shutdown.is_none() {
							anyhow::bail!("ingest closed");
						}

						break;
					};

					self.handle_msg(global, msg.context("ingest recv failed")?).await?;
				},
			}

			self.check_tasks()?;
		}

		Ok(())
	}

	fn check_tasks(&mut self) -> Result<()> {
		if let Some(task) = self.tasks.iter_mut().find(|task| task.is_finished()) {
			anyhow::bail!("task exited early: {}", task.tag());
		}

		Ok(())
	}

	fn ready(&mut self) -> Result<()> {
		if self.transcoder_ready || !self.ingest_ready && self.screenshot_idx != 0 {
			return Ok(());
		}

		self.transcoder_ready = true;
		self.generic_uploader
			.try_send(GenericTask::RoomReady)
			.context("send room ready task")?;

		Ok(())
	}

	async fn handle_msg(&mut self, global: &Arc<impl TranscoderGlobal>, msg: IngestWatchResponse) -> Result<()> {
		let msg = msg.message.ok_or_else(|| anyhow::anyhow!("ingest sent bad message"))?;

		match msg {
			ingest_watch_response::Message::Media(media) => {
				let mut outputs = Vec::new();
				{
					let input = self
						.ffmpeg_send
						.as_ref()
						.ok_or_else(|| anyhow::anyhow!("ffmpeg already shutdown"))?;
					let mut ffmpeg_input_fut = pin!(input.send(media.data.clone()));
					loop {
						select! {
							_ = &mut ffmpeg_input_fut => {
								break;
							},
							Some(output) = self.ffmpeg_recv.recv() => {
								outputs.push(output);
							}
						}
					}
				}

				outputs
					.into_iter()
					.try_for_each(|(rendition, track_out)| self.handle_track(rendition, track_out))?;
			}
			ingest_watch_response::Message::Shutdown(s) => {
				self.ingest_shutdown = Some(
					ingest_watch_response::Shutdown::try_from(s).unwrap_or(ingest_watch_response::Shutdown::Transcoder),
				);
				self.ffmpeg_send.take();
			}
			ingest_watch_response::Message::Ready(_) => {
				self.ingest_ready = true;
				self.fetch_manifests(global).await?;
				self.put_init_segments()?;
				tracing::info!("ingest reported ready");
			}
		}

		Ok(())
	}

	fn handle_track(&mut self, rendition: Rendition, track_out: TrackOut) -> Result<()> {
		let track = self.tracks.get_mut(&rendition).unwrap();

		let update_manifest = track.handle_track_out(self.recording.as_mut(), track_out)?;

		self.put_init_segments()?;

		if update_manifest && !self.first_init_put {
			let info_map = self.track_info_map();
			self.tracks
				.get_mut(&rendition)
				.unwrap()
				.update_manifest(self.recording.as_mut(), &info_map, false)?;
		}

		Ok(())
	}

	fn put_init_segments(&mut self) -> Result<()> {
		if !self.first_init_put || !self.ingest_ready || self.tracks.iter().any(|(_, state)| state.init_segment().is_none())
		{
			return Ok(());
		}

		self.first_init_put = false;

		self.tracks
			.values_mut()
			.try_for_each(|track| track.ready(self.recording.as_mut()))?;

		let info_map = self.track_info_map();
		self.tracks
			.values_mut()
			.try_for_each(|track| track.update_manifest(self.recording.as_mut(), &info_map, false))?;

		if let Some(recording) = &mut self.recording {
			self.tracks.iter().try_for_each(|(rendition, state)| {
				recording.upload_init(*rendition, state.init_segment().unwrap().clone())
			})?;
		}

		self.ready()?;

		Ok(())
	}

	fn track_info_map(&self) -> HashMap<String, RenditionInfo> {
		self.tracks
			.iter()
			.map(|(rendition, ts)| (rendition.to_string(), ts.info()))
			.collect()
	}

	pub async fn handle_shutdown(mut self) -> Result<()> {
		tracing::info!("shutting down transcoder");

		drop(self.ffmpeg_send.take());

		while let Some((rendition, track_out)) = self.ffmpeg_recv.recv().await {
			self.handle_track(rendition, track_out)?;
		}

		let is_shutdown = self.ingest_shutdown == Some(ingest_watch_response::Shutdown::Stream);

		self.tracks
			.values_mut()
			.try_for_each(|track| track.finish(self.recording.as_mut()))?;

		let info_map = self
			.tracks
			.iter()
			.map(|(rendition, ts)| (rendition.to_string(), ts.info()))
			.collect();

		self.update_manifest()?;

		self.tracks
			.drain()
			.try_for_each(|(_, mut track)| track.update_manifest(self.recording.as_mut(), &info_map, is_shutdown))?;

		// Close the generic uploader so that it can finish its tasks
		drop(self.generic_uploader);

		// New tasks may have been added during shutdown, so we need to check again
		for mut task in self.tasks.drain(..) {
			task.join()
				.await
				.with_context(|| format!("{}: panic'd", task.tag()))?
				.with_context(|| format!("{}: ", task.tag()))?;
		}

		if let Some(shutdown) = self.ingest_shutdown.take() {
			match shutdown {
				ingest_watch_response::Shutdown::Stream => {}
				ingest_watch_response::Shutdown::Transcoder => {
					self.ingest_send.try_send(IngestWatchRequest {
						message: Some(ingest_watch_request::Message::Shutdown(
							ingest_watch_request::Shutdown::Complete as i32,
						)),
					})?;
				}
			}
		}

		Ok(())
	}

	fn update_manifest(&mut self) -> Result<()> {
		if !self.ingest_ready {
			return Ok(());
		}

		let data = LiveManifest {
			screenshot_idx: self.screenshot_idx,
		}
		.encode_to_vec()
		.into();

		self.generic_uploader
			.try_send(GenericTask::Manifest { data })
			.context("send screenshot task")?;

		self.tracks.values_mut().try_for_each(|track| track.upload_init())?;

		Ok(())
	}

	async fn fetch_manifests(&mut self, global: &Arc<impl TranscoderGlobal>) -> Result<()> {
		let rendition_manfiests = async {
			futures_util::future::try_join_all(self.tracks.keys().map(|rendition| {
				global
					.metadata_store()
					.get(video_common::keys::rendition_manifest(
						self.organization_id,
						self.room_id,
						self.connection_id,
						*rendition,
					))
					.map_ok(|v| (*rendition, v))
			}))
			.await
		};

		let manifest = async {
			global
				.metadata_store()
				.get(video_common::keys::manifest(
					self.organization_id,
					self.room_id,
					self.connection_id,
				))
				.await
		};

		let (rendition_manfiests, manifest) = try_join!(rendition_manfiests, manifest)?;

		if rendition_manfiests.iter().all(|(_, v)| v.is_none()) && manifest.is_none() {
			return Ok(());
		}

		let Some(manifest) = manifest else {
			anyhow::bail!("missing manifest");
		};

		let manifest = LiveManifest::decode(manifest)?;

		self.screenshot_idx = manifest.screenshot_idx;

		for (rendition, data) in rendition_manfiests {
			let Some(data) = data else {
				anyhow::bail!("missing manifest for rendition {}", rendition);
			};

			let manifest = LiveRenditionManifest::decode(data)?;

			if let Some(recording) = &mut self.recording {
				if let Some(data) = &manifest.recording_data {
					recording.recover_thumbnails(data.thumbnails.clone());
				}
			}

			self.tracks.get_mut(&rendition).unwrap().apply_manifest(manifest);
		}

		Ok(())
	}
}
