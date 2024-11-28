use std::net::IpAddr;
use std::pin::pin;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use base64::Engine;
use bytes::Bytes;
use bytesio::bytesio::AsyncReadWrite;
use flv::{FlvTag, FlvTagData, FlvTagType};
use futures::Future;
use futures_util::StreamExt;
use pb::scuffle::video::internal::events::TranscoderRequestTask;
use pb::scuffle::video::internal::{ingest_watch_request, ingest_watch_response, IngestWatchRequest, IngestWatchResponse};
use pb::scuffle::video::v1::events_fetch_request::Target;
use pb::scuffle::video::v1::types::{event, Rendition};
use prost::Message as _;
use rtmp::{ChannelData, PublishRequest, Session, SessionError};
use scuffle_utils::context::ContextExt;
use scuffle_utils::prelude::FutureTimeout;
use tokio::select;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tonic::{Status, Streaming};
use transmuxer::{AudioSettings, MediaSegment, TransmuxResult, Transmuxer, VideoSettings};
use ulid::Ulid;
use video_common::database::RoomStatus;
use video_common::{events, keys};

use super::bytes_tracker::BytesTracker;
use super::errors::IngestError;
use super::rtmp_session::{Data, RtmpSession};
use super::update::{update_db, Update};
use crate::config::IngestConfig;
use crate::global::{IncomingTranscoder, IngestGlobal};

struct Transcoder {
	send: mpsc::Sender<IngestWatchResponse>,
	recv: Streaming<IngestWatchRequest>,
}

struct Connection {
	id: Ulid,

	bytes_tracker: BytesTracker,
	initial_segment: Option<Bytes>,
	fragment_list: Vec<MediaSegment>,

	transmuxer: Transmuxer,

	current_transcoder_id: Ulid,
	next_transcoder_id: Option<Ulid>,

	incoming_reciever: mpsc::Receiver<IncomingTranscoder>,
	incoming_sender: mpsc::Sender<IncomingTranscoder>,

	update_sender: Option<mpsc::Sender<Update>>,
	update_recv: Option<mpsc::Receiver<Update>>,

	current_transcoder: Option<Transcoder>, // The current main transcoder
	next_transcoder: Option<Transcoder>,    // The next transcoder to be used
	old_transcoder: Option<Transcoder>,     // The old transcoder that is being replaced

	last_transcoder_publish: Instant,
	last_keyframe: Instant,

	video_timescale: u32,
	audio_timescale: u32,

	error: Option<IngestError>,

	// The room that is being published to
	organization_id: Ulid,
	room_id: Ulid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WhichTranscoder {
	Current,
	Next,
	Old,
}

#[tracing::instrument(skip(global, socket))]
pub async fn handle<G: IngestGlobal, S: AsyncReadWrite>(global: Arc<G>, socket: S, ip: IpAddr) {
	// We only need a single buffer channel for this session because the entire
	// session is single threaded and we don't need to worry about buffering.
	let (event_producer, publish) = mpsc::channel(1);
	let (data_producer, data) = mpsc::channel(1);

	let mut session = Session::new(socket, data_producer, event_producer);

	// When a future is pinned it becomes pausable and can be resumed later
	// The entire design here is to run on a single task, and share execution on the
	// single thread. So when we select on this future we are allowing this future
	// to execute. This makes it so the session cannot run outside of its turn.
	// Essentially this is how tokio's executor works, but we are doing it manually.
	// This also has the advantage of being completely cleaned up when the function
	// goes out of scope. If we used a tokio::spawn here, we would have to manually
	// clean up the task.
	let fut = pin!(session.run());
	let mut session = RtmpSession::new(fut, publish, data);

	let Ok(Ok(Ok(Some(event)))) = session.publish().context(global.ctx()).timeout(Duration::from_secs(5)).await else {
		tracing::debug!("connection disconnected before publish");
		return;
	};

	let mut connection = match Connection::new(&global, event, ip).await {
		Ok(Some(c)) => c,
		Ok(None) => return,
		Err(e) => {
			tracing::error!(error = %e, "failed to create connection");
			return;
		}
	};

	let clean_disconnect = connection.run(&global, session).await;

	if let Err(err) = connection.cleanup(&global, clean_disconnect).await {
		tracing::error!(error = %err, "failed to cleanup connection")
	}
}

impl Connection {
	#[tracing::instrument(
        level = "debug",
        skip(global, event, _ip),
        fields(app = %event.app_name, stream = %event.stream_name)
    )]
	async fn new<G: IngestGlobal>(global: &Arc<G>, event: PublishRequest, _ip: IpAddr) -> Result<Option<Self>> {
		if event.app_name != "live" {
			return Ok(None);
		}

		let mut parts = event.stream_name.split('_');
		if parts.next() != Some("live") {
			return Ok(None);
		}

		let organization_id = match parts.next().and_then(|id| Ulid::from_string(id).ok()) {
			Some(id) => id,
			None => return Ok(None),
		};

		let parse_room_secret = |name: &str| {
			if name.len() > 512 {
				return None;
			}

			let name = base64::engine::general_purpose::URL_SAFE_NO_PAD
				.decode(name.as_bytes())
				.ok()?;

			let room_name_secret = std::str::from_utf8(&name).ok()?;

			let mut parts = room_name_secret.split('+');
			let room_id = Ulid::from_string(parts.next()?).ok()?;
			let room_secret = parts.next()?;
			if parts.next().is_some() {
				return None;
			}

			Some((room_id, room_secret.to_string()))
		};

		let (room_id, room_secret) = match parts.next().and_then(parse_room_secret) {
			Some(name) => name,
			None => return Ok(None),
		};

		#[derive(postgres_from_row::FromRow)]
		struct Response {
			id: Option<Ulid>,
		}

		let id = Ulid::new();

		let result: Option<Response> = scuffle_utils::database::query(
			r#"
            UPDATE rooms as new
            SET 
                updated_at = NOW(),
                last_live_at = NOW(),
                last_disconnected_at = NULL,
                active_ingest_connection_id = $1,
                status = $2,
                video_input = NULL,
                audio_input = NULL,
                ingest_bitrate = NULL,
                video_output = NULL,
                audio_output = NULL,
                active_recording_id = NULL,
                active_recording_config = NULL,
                active_transcoding_config = NULL
            FROM rooms as old
            WHERE 
                new.organization_id = $3 AND
                new.id = $4 AND
                new.stream_key = $5 AND
                (new.last_live_at < NOW() - INTERVAL '10 seconds' OR new.last_live_at IS NULL) AND
                old.organization_id = new.organization_id AND
                old.id = new.id AND
                old.stream_key = new.stream_key
            RETURNING old.active_ingest_connection_id as id
            "#,
		)
		.bind(id)
		.bind(RoomStatus::Offline)
		.bind(organization_id)
		.bind(room_id)
		.bind(&room_secret)
		.build_query_as()
		.fetch_optional(global.db())
		.await?;

		let Some(result) = result else {
			tracing::debug!("failed to find room");
			return Ok(None);
		};

		if let Some(old_id) = result.id {
			if let Err(err) = global.nats().publish(keys::ingest_disconnect(old_id), Bytes::new()).await {
				tracing::error!(error = %err, "failed to publish disconnect event");
			}
		}

		event.response.send(id.into()).ok();

		let (update_sender, update_reciever) = mpsc::channel(15);
		let (incoming_sender, incoming_reciever) = mpsc::channel(15);

		Ok(Some(Connection {
			id,
			transmuxer: Transmuxer::new(),
			bytes_tracker: BytesTracker::default(),
			current_transcoder_id: Ulid::nil(),
			current_transcoder: None,
			next_transcoder_id: None,
			old_transcoder: None,
			next_transcoder: None,
			initial_segment: None,
			fragment_list: Vec::new(),
			last_transcoder_publish: Instant::now(),
			last_keyframe: Instant::now(),
			update_sender: Some(update_sender),
			update_recv: Some(update_reciever),
			incoming_sender,
			incoming_reciever,
			organization_id,
			room_id,
			video_timescale: 1,
			audio_timescale: 1,
			error: None,
		}))
	}

	#[tracing::instrument(
        level = "info",
        skip(self, global, session),
        fields(organization_id = %self.organization_id, room_id = %self.room_id)
    )]
	async fn run<'a, G: IngestGlobal>(
		&mut self,
		global: &Arc<G>,
		mut session: RtmpSession<'a, impl Future<Output = Result<bool, SessionError>>>,
	) -> bool {
		tracing::info!("new publish request");

		// At this point we have a stream that is publishing to us
		// We can now poll the run future & the data receiver.
		// The run future will close when the connection is closed or an error occurs
		// The data receiver will never close, because the Session object is always in
		// scope.

		let mut bitrate_update_interval = tokio::time::interval(global.config().bitrate_update_interval);
		bitrate_update_interval.tick().await; // Skip the first tick (resolves instantly)

		let mut db_update_fut = pin!(update_db(
			global.clone(),
			self.id,
			self.organization_id,
			self.room_id,
			self.update_recv.take().unwrap(),
		));

		let mut next_timeout = Instant::now() + Duration::from_secs(2);

		let mut clean_shutdown = false;

		let mut conn_id_sub = match global.nats().subscribe(keys::ingest_disconnect(self.id)).await {
			Ok(sub) => sub,
			Err(e) => {
				tracing::error!(error = %e, "failed to subscribe to disconnect subject");

				self.error = Some(IngestError::FailedToSubscribe);

				return false;
			}
		};

		while select! {
			_ = global.ctx().done() => {
				tracing::debug!("Global context closed, closing connection");

				self.error = Some(IngestError::IngestShutdown);

				false
			},
			d = session.data() => {
				match d {
					Err(e) => {
						tracing::error!(error = %e, "session error");

						self.error = Some(IngestError::RtmpConnectionError);

						false
					},
					Ok(Data::Data(data)) => {
						next_timeout = Instant::now() + Duration::from_secs(2);
						self.on_data(global, data.expect("data producer closed")).await
					}
					Ok(Data::Closed(c)) => {
						clean_shutdown = c;

						tracing::debug!("session closed: {c}");

						false
					}
				}
			},
			m = conn_id_sub.next() => {
				if m.is_none() {
					tracing::error!("connection id subject closed");

					self.error = Some(IngestError::SubscriptionClosedUnexpectedly);

					false
				} else {
					tracing::debug!("disconnect requested");

					self.error = Some(IngestError::DisconnectRequested);
					clean_shutdown = true;

					false
				}
			},
			_ = bitrate_update_interval.tick() => self.on_bitrate_update(global),
			_ = tokio::time::sleep_until(next_timeout) => {
				tracing::debug!("session timed out during data");

				self.error = Some(IngestError::RtmpConnectionTimeout);

				false
			},
			_ = &mut db_update_fut => {
				tracing::error!("api update future failed");

				self.error = Some(IngestError::FailedToUpdateBitrate);

				false
			}
			Some(msg) = async {
				if let Some(transcoder) = self.current_transcoder.as_mut() {
					Some(transcoder.recv.message().await)
				} else {
					None
				}
			} => {
				// handle message from transcoder
				self.handle_transcoder_message(global, msg, WhichTranscoder::Current).await
			}
			Some(msg) = async {
				if let Some(transcoder) = self.next_transcoder.as_mut() {
					Some(transcoder.recv.message().await)
				} else {
					None
				}
			} => {
				// handle message from transcoder
				self.handle_transcoder_message(global, msg, WhichTranscoder::Next).await
			}
			Some(msg) = async {
				if let Some(transcoder) = self.old_transcoder.as_mut() {
					Some(transcoder.recv.message().await)
				} else {
					None
				}
			} => {
				// handle message from transcoder
				self.handle_transcoder_message(global, msg, WhichTranscoder::Old).await
			}
			event = self.incoming_reciever.recv() => self.handle_incoming_request(event.expect("transcoder closed")).await,
		} {}

		self.update_sender.take();

		tracing::info!(clean = clean_shutdown, "connection closed");

		clean_shutdown
	}

	async fn handle_transcoder_message<G: IngestGlobal>(
		&mut self,
		global: &Arc<G>,
		msg: Result<Option<IngestWatchRequest>, Status>,
		transcoder: WhichTranscoder,
	) -> bool {
		match msg {
			Ok(Some(msg)) => {
				match msg.message {
					Some(ingest_watch_request::Message::Shutdown(shutdown)) => {
						match ingest_watch_request::Shutdown::try_from(shutdown).unwrap_or_default() {
							ingest_watch_request::Shutdown::Request => {}
							ingest_watch_request::Shutdown::Complete => {
								if transcoder == WhichTranscoder::Old {
									self.old_transcoder = None;

									events::emit(
										global.nats(),
										&global.config().events_stream_name,
										self.organization_id,
										Target::Room,
										event::Event::Room(event::Room {
											room_id: Some(self.room_id.into()),
											event: Some(event::room::Event::TranscoderDisconnected(
												event::room::TranscoderDisconnected {
													connection_id: Some(self.id.into()),
													clean: true,
												},
											)),
										}),
									)
									.await;

									if let Some(transcoder) = &mut self.current_transcoder {
										if transcoder
											.send
											.send(IngestWatchResponse {
												message: Some(ingest_watch_response::Message::Ready(
													ingest_watch_response::Ready::Ready as i32,
												)),
											})
											.await
											.is_err()
										{
											tracing::error!("failed to send ready message to transcoder");
										} else {
											return true;
										}
									} else {
										return true;
									}
								} else {
									tracing::warn!("transcoder sent shutdown message before we requested it");
								}
							}
						}
					}
					_ => {
						tracing::warn!("transcoder sent an unknwon message");
						return true;
					}
				}

				if transcoder == WhichTranscoder::Next {
					self.next_transcoder_id = None;
					self.next_transcoder = None;
				}
			}
			Err(_) | Ok(None) => {
				tracing::warn!("transcoder seems to have disconnected unexpectedly");

				events::emit(
					global.nats(),
					&global.config().events_stream_name,
					self.organization_id,
					Target::Room,
					event::Event::Room(event::Room {
						room_id: Some(self.room_id.into()),
						event: Some(event::room::Event::TranscoderDisconnected(
							event::room::TranscoderDisconnected {
								connection_id: Some(self.id.into()),
								clean: false,
							},
						)),
					}),
				)
				.await;

				match transcoder {
					WhichTranscoder::Current => {
						self.current_transcoder = None;
						self.current_transcoder_id = Ulid::nil();
						match scuffle_utils::database::query(
							r#"
                            UPDATE rooms
                            SET 
                                updated_at = NOW(),
                                status = $1
                            WHERE
                                organization_id = $2 AND 
                                id = $3 AND
                                active_ingest_connection_id = $4
                            "#,
						)
						.bind(RoomStatus::WaitingForTranscoder)
						.bind(self.organization_id)
						.bind(self.room_id)
						.bind(self.id)
						.build()
						.execute(global.db())
						.await
						{
							Ok(r) => {
								if r != 1 {
									tracing::error!("failed to update room status");

									self.error = Some(IngestError::FailedToUpdateRoom);

									return false;
								}
							}
							Err(e) => {
								tracing::error!(error = %e, "failed to update room status");

								self.error = Some(IngestError::FailedToUpdateRoom);

								return false;
							}
						}
					}
					WhichTranscoder::Old => {
						self.old_transcoder = None;
						if let Some(transcoder) = &mut self.current_transcoder {
							if let Err(err) = transcoder
								.send
								.send(IngestWatchResponse {
									message: Some(ingest_watch_response::Message::Ready(
										ingest_watch_response::Ready::Ready as i32,
									)),
								})
								.await
							{
								tracing::error!(error = %err, "failed to send ready message to transcoder");
							} else {
								return true;
							}
						} else {
							return true;
						}
					}
					WhichTranscoder::Next => {
						self.next_transcoder_id = None;
						self.next_transcoder = None;
					}
				}
			}
		}

		if matches!(transcoder, WhichTranscoder::Current | WhichTranscoder::Next) {
			self.request_transcoder(global).await
		} else {
			true
		}
	}

	async fn handle_incoming_request(&mut self, event: IncomingTranscoder) -> bool {
		let Some(init_segment) = &self.initial_segment else {
			tracing::error!("out of order events, requested transcoder before init segment");
			return false;
		};

		if Some(event.ulid) != self.next_transcoder_id {
			tracing::warn!("got incoming request from transcoder that we didn't request");
			return true;
		}

		if event
			.transcoder
			.try_send(IngestWatchResponse {
				message: Some(ingest_watch_response::Message::Media(ingest_watch_response::Media {
					r#type: ingest_watch_response::media::Type::Init.into(),
					data: init_segment.clone(),
					keyframe: false,
					timestamp: 0,
					timescale: 1,
				})),
			})
			.is_err()
		{
			tracing::warn!("transcoder disconnected before we could send init segment");
			return true;
		}

		if self.current_transcoder.is_none() && !self.fragment_list.is_empty() {
			if event
				.transcoder
				.try_send(IngestWatchResponse {
					message: Some(ingest_watch_response::Message::Ready(
						ingest_watch_response::Ready::Ready.into(),
					)),
				})
				.is_err()
			{
				tracing::warn!("transcoder disconnected before we could send init segment");
				return true;
			}

			self.current_transcoder = Some(Transcoder {
				recv: event.streaming,
				send: event.transcoder,
			});
			self.current_transcoder_id = self.next_transcoder_id.take().unwrap();
		} else {
			self.next_transcoder = Some(Transcoder {
				recv: event.streaming,
				send: event.transcoder,
			});
		}

		true
	}

	fn send_update(&mut self, update: Update) -> bool {
		if let Some(sender) = &mut self.update_sender {
			sender.try_send(update).is_ok()
		} else {
			false
		}
	}

	async fn request_transcoder<G: IngestGlobal>(&mut self, global: &Arc<G>) -> bool {
		// If we already have a request pending, then we don't need to request another
		// one.
		if self.next_transcoder_id.is_some() {
			return true;
		}

		let request_id = Ulid::new();
		self.next_transcoder_id = Some(request_id);

		global
			.requests()
			.lock()
			.await
			.insert(request_id, self.incoming_sender.clone());

		let config = global.config::<IngestConfig>();

		if let Err(err) = global
			.nats()
			.publish(
				config.transcoder_request_subject.clone(),
				TranscoderRequestTask {
					organization_id: Some(self.organization_id.into()),
					room_id: Some(self.room_id.into()),
					request_id: Some(request_id.into()),
					connection_id: Some(self.id.into()),
					grpc_endpoint: config.grpc_advertise_address.clone(),
				}
				.encode_to_vec()
				.into(),
			)
			.await
		{
			tracing::error!(error = %err, "failed to publish transcoder request");

			self.error = Some(IngestError::FailedToRequestTranscoder);

			return false;
		}

		tracing::info!("requested transcoder");

		true
	}

	async fn on_init_segment<G: IngestGlobal>(
		&mut self,
		global: &Arc<G>,
		video_settings: &VideoSettings,
		audio_settings: &AudioSettings,
		init_data: Bytes,
	) -> bool {
		self.initial_segment = Some(init_data);

		self.audio_timescale = audio_settings.timescale;
		self.video_timescale = video_settings.timescale;

		let video_settings = pb::scuffle::video::v1::types::VideoConfig {
			bitrate: video_settings.bitrate as i64,
			codec: video_settings.codec.to_string(),
			fps: video_settings.framerate as i32,
			height: video_settings.height as i32,
			width: video_settings.width as i32,
			rendition: Rendition::VideoSource.into(),
		}
		.encode_to_vec();

		let audio_settings = pb::scuffle::video::v1::types::AudioConfig {
			bitrate: audio_settings.bitrate as i64,
			channels: audio_settings.channels as i32,
			codec: audio_settings.codec.to_string(),
			sample_rate: audio_settings.sample_rate as i32,
			rendition: Rendition::AudioSource.into(),
		}
		.encode_to_vec();

		match scuffle_utils::database::query(
			r#"
			UPDATE rooms
			SET 
				updated_at = NOW(),
				status = $1,
				video_input = $2,
				audio_input = $3
			WHERE
				organization_id = $4 AND 
				id = $5 AND
				active_ingest_connection_id = $6
			"#,
		)
		.bind(RoomStatus::WaitingForTranscoder)
		.bind(video_settings)
		.bind(audio_settings)
		.bind(self.organization_id)
		.bind(self.room_id)
		.bind(self.id)
		.build()
		.execute(global.db())
		.await
		{
			Ok(r) => {
				if r != 1 {
					tracing::error!("failed to update room status");

					self.error = Some(IngestError::FailedToUpdateRoom);

					return false;
				}
			}
			Err(e) => {
				tracing::error!(error = %e, "failed to update room status");

				self.error = Some(IngestError::FailedToUpdateRoom);

				return false;
			}
		}

		video_common::events::emit(
			global.nats(),
			&global.config().events_stream_name,
			self.organization_id,
			Target::Room,
			event::Event::Room(event::Room {
				room_id: Some(self.room_id.into()),
				event: Some(event::room::Event::Connected(event::room::Connected {
					connection_id: Some(self.id.into()),
				})),
			}),
		)
		.await;

		self.request_transcoder(global).await
	}

	async fn on_data<G: IngestGlobal>(&mut self, global: &Arc<G>, data: ChannelData) -> bool {
		self.bytes_tracker.add(&data);

		let config = global.config::<IngestConfig>();

		if self.bytes_tracker.since_keyframe() >= config.max_bytes_between_keyframes {
			self.error = Some(IngestError::KeyframeBitrateDistance(
				self.bytes_tracker.since_keyframe(),
				config.max_bytes_between_keyframes,
			));

			tracing::debug!(
				"keyframe bitrate distance exceeded: {:?} - {} > {}",
				Instant::now() - self.last_keyframe,
				self.bytes_tracker.since_keyframe(),
				config.max_bytes_between_keyframes
			);

			return false;
		}

		if self.bytes_tracker.total() * 8 >= config.max_bitrate * config.bitrate_update_interval.as_secs() {
			self.error = Some(IngestError::BitrateLimit(
				self.bytes_tracker.total() / config.bitrate_update_interval.as_secs() * 8,
				config.max_bitrate,
			));

			tracing::debug!(
				"bitrate limit exceeded: {} > {}",
				self.bytes_tracker.total() * 8 / config.bitrate_update_interval.as_secs(),
				config.max_bitrate
			);

			return false;
		}

		match data {
			ChannelData::Video { data, timestamp } => {
				let data = match FlvTagData::demux(FlvTagType::Video as u8, data) {
					Ok(data) => data,
					Err(e) => {
						tracing::error!(error = %e, "demux error");

						self.error = Some(IngestError::VideoDemux);

						return false;
					}
				};

				self.transmuxer.add_tag(FlvTag {
					timestamp,
					data,
					stream_id: 0,
				});
			}
			ChannelData::Audio { data, timestamp } => {
				let data = match FlvTagData::demux(FlvTagType::Audio as u8, data) {
					Ok(data) => data,
					Err(e) => {
						tracing::error!(error = %e, "demux error");

						self.error = Some(IngestError::AudioDemux);

						return false;
					}
				};

				self.transmuxer.add_tag(FlvTag {
					timestamp,
					data,
					stream_id: 0,
				});
			}
			ChannelData::Metadata { data, timestamp } => {
				let data = match FlvTagData::demux(FlvTagType::ScriptData as u8, data) {
					Ok(data) => data,
					Err(e) => {
						tracing::error!(error = %e, "demux error");

						self.error = Some(IngestError::MetadataDemux);

						return false;
					}
				};

				self.transmuxer.add_tag(FlvTag {
					timestamp,
					data,
					stream_id: 0,
				});
			}
		}

		// We need to check if the transmuxer has any packets ready to be muxed
		match self.transmuxer.mux() {
			Ok(Some(TransmuxResult::InitSegment {
				video_settings,
				audio_settings,
				data,
			})) => {
				let bitrate = video_settings.bitrate as u64 + audio_settings.bitrate as u64;
				if bitrate >= config.max_bitrate {
					self.error = Some(IngestError::BitrateLimit(bitrate, config.max_bitrate));

					tracing::debug!("bitrate limit exceeded: {} > {}", bitrate, config.max_bitrate);

					return false;
				}

				self.on_init_segment(global, &video_settings, &audio_settings, data).await
			}
			Ok(Some(TransmuxResult::MediaSegment(segment))) => self.on_media_segment(global, segment).await,
			Ok(None) => true,
			Err(e) => {
				tracing::error!(error = %e, "error muxing packet");

				self.error = Some(IngestError::Mux);

				false
			}
		}
	}

	pub async fn on_media_segment<G: IngestGlobal>(&mut self, global: &Arc<G>, segment: MediaSegment) -> bool {
		let config = global.config::<IngestConfig>();

		if Instant::now() - self.last_keyframe >= config.max_time_between_keyframes {
			self.error = Some(IngestError::KeyframeTimeLimit(config.max_time_between_keyframes.as_secs()));

			tracing::debug!(
				"keyframe time limit exceeded: {:?} > {:?}",
				Instant::now() - self.last_keyframe,
				config.max_time_between_keyframes
			);

			return false;
		}

		if Instant::now() - self.last_transcoder_publish >= config.transcoder_timeout {
			tracing::error!("no transcoder available to publish to");

			self.error = Some(IngestError::NoTranscoderAvailable);

			return false;
		}

		if segment.keyframe {
			self.last_keyframe = Instant::now();

			self.bytes_tracker.keyframe();

			if let Some(transcoder) = self.next_transcoder.take() {
				if let Some(transcoder) = self.current_transcoder.take() {
					transcoder
						.send
						.send(IngestWatchResponse {
							message: Some(ingest_watch_response::Message::Shutdown(
								ingest_watch_response::Shutdown::Transcoder as i32,
							)),
						})
						.await
						.ok();
					self.old_transcoder = Some(transcoder);
				}

				if self.old_transcoder.is_none()
					&& transcoder
						.send
						.send(IngestWatchResponse {
							message: Some(ingest_watch_response::Message::Ready(
								ingest_watch_response::Ready::Ready as i32,
							)),
						})
						.await
						.is_err()
				{
					tracing::error!("transcoder disconnected while sending fragment");
					if !self.request_transcoder(global).await {
						return false;
					}
				}

				self.current_transcoder = Some(transcoder);
				self.current_transcoder_id = self.next_transcoder_id.take().unwrap();
			};
		}

		if let Some(transcoder) = &mut self.current_transcoder {
			let mut failed = false;
			for fragment in &self.fragment_list {
				if transcoder
					.send
					.send(IngestWatchResponse {
						message: Some(ingest_watch_response::Message::Media(ingest_watch_response::Media {
							r#type: match fragment.ty {
								transmuxer::MediaType::Audio => ingest_watch_response::media::Type::Audio as i32,
								transmuxer::MediaType::Video => ingest_watch_response::media::Type::Video as i32,
							},
							data: fragment.data.clone(),
							keyframe: fragment.keyframe,
							timestamp: fragment.timestamp,
							timescale: match fragment.ty {
								transmuxer::MediaType::Audio => self.audio_timescale,
								transmuxer::MediaType::Video => self.video_timescale,
							},
						})),
					})
					.await
					.is_err()
				{
					failed = true;
					break;
				}

				self.last_transcoder_publish = Instant::now();
			}

			if !failed
				&& transcoder
					.send
					.send(IngestWatchResponse {
						message: Some(ingest_watch_response::Message::Media(ingest_watch_response::Media {
							r#type: match segment.ty {
								transmuxer::MediaType::Audio => ingest_watch_response::media::Type::Audio as i32,
								transmuxer::MediaType::Video => ingest_watch_response::media::Type::Video as i32,
							},
							keyframe: segment.keyframe,
							data: segment.data.clone(),
							timestamp: segment.timestamp,
							timescale: match segment.ty {
								transmuxer::MediaType::Audio => self.audio_timescale,
								transmuxer::MediaType::Video => self.video_timescale,
							},
						})),
					})
					.await
					.is_ok()
			{
				self.last_transcoder_publish = Instant::now();
				self.fragment_list.clear();

				return true;
			}

			tracing::error!("transcoder disconnected while sending fragment");
			if !self.request_transcoder(global).await {
				return false;
			}
		}

		if segment.keyframe && segment.ty == transmuxer::MediaType::Video {
			self.fragment_list.clear();
			self.fragment_list.push(segment);
		} else if !self.fragment_list.is_empty() {
			self.fragment_list.push(segment);
		}

		true
	}

	fn on_bitrate_update<G: IngestGlobal>(&mut self, global: &Arc<G>) -> bool {
		let bitrate = self.bytes_tracker.total() / global.config::<IngestConfig>().bitrate_update_interval.as_secs();

		self.bytes_tracker.clear();

		if !self.send_update(Update { bitrate: bitrate as i64 }) {
			self.error = Some(IngestError::FailedToUpdateBitrate);
			tracing::error!("failed to send bitrate update");
			false
		} else {
			true
		}
	}

	async fn cleanup<G: IngestGlobal>(&mut self, global: &Arc<G>, clean_disconnect: bool) -> Result<()> {
		if let Some(next_id) = self.next_transcoder_id.take() {
			global.requests().lock().await.remove(&next_id);
		}

		if !self.current_transcoder_id.is_nil() {
			global.requests().lock().await.remove(&self.current_transcoder_id);
		}

		if let Some(transcoder) = self.next_transcoder.take() {
			transcoder
				.send
				.try_send(IngestWatchResponse {
					message: Some(ingest_watch_response::Message::Shutdown(
						ingest_watch_response::Shutdown::Stream as i32,
					)),
				})
				.ok();
		}

		if let Some(transcoder) = self.current_transcoder.take() {
			transcoder
				.send
				.try_send(IngestWatchResponse {
					message: Some(ingest_watch_response::Message::Shutdown(
						ingest_watch_response::Shutdown::Stream as i32,
					)),
				})
				.ok();
		}

		video_common::events::emit(
			global.nats(),
			&global.config().events_stream_name,
			self.organization_id,
			Target::Room,
			event::Event::Room(event::Room {
				room_id: Some(self.room_id.into()),
				event: Some(event::room::Event::Disconnected(event::room::Disconnected {
					connection_id: Some(self.id.into()),
					clean: clean_disconnect,
					cause: self.error.as_ref().map(|e| e.to_string()),
				})),
			}),
		)
		.await;

		scuffle_utils::database::query(
			r#"
			UPDATE rooms
			SET
				updated_at = NOW(),
				last_disconnected_at = NOW(),
				active_ingest_connection_id = NULL,
				video_input = NULL,
				audio_input = NULL,
				ingest_bitrate = NULL,
				video_output = NULL,
				audio_output = NULL,
				active_recording_id = NULL,
				active_recording_config = NULL,
				active_transcoding_config = NULL,
				status = $1
				WHERE 
					organization_id = $2 AND
					id = $3 AND
					active_ingest_connection_id = $4
			"#,
		)
		.bind(RoomStatus::Offline)
		.bind(self.organization_id)
		.bind(self.room_id)
		.bind(self.id)
		.build()
		.execute(global.db())
		.await?;

		Ok(())
	}
}
