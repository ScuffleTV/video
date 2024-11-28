use std::ops::Sub;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use async_stream::stream;
use base64::Engine;
use binary_helper::config::TlsConfig;
use binary_helper::global::*;
use bytes::Bytes;
use futures::StreamExt;
use pb::ext::UlidExt;
use pb::scuffle::video::internal::events::TranscoderRequestTask;
use pb::scuffle::video::internal::ingest_client::IngestClient;
use pb::scuffle::video::internal::{ingest_watch_request, ingest_watch_response, IngestWatchRequest, IngestWatchResponse};
use pb::scuffle::video::v1::events_fetch_request::Target;
use pb::scuffle::video::v1::types::{event, Event, Rendition};
use prost::Message;
use scuffle_utils::context::ContextExt;
use scuffle_utils::prelude::FutureTimeout;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use ulid::Ulid;
use uuid::Uuid;
use video_common::database::Room;
use video_common::keys::{self, event_subject};

use super::global::GlobalState;
use crate::config::{IngestConfig, RtmpConfig};
use crate::tests::global::mock_global_state;

fn generate_key(org_id: Ulid, room_id: Ulid) -> String {
	format!(
		"live_{}_{}",
		org_id,
		base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(format!("{}+{}", room_id, room_id))
	)
}

fn stream_with_ffmpeg(rtmp_port: u16, file: &str, key: &str) -> tokio::process::Child {
	let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../assets");

	Command::new("ffmpeg")
		.args([
			"-re",
			"-i",
			dir.join(file).to_str().expect("failed to get path"),
			"-c",
			"copy",
			"-f",
			"flv",
			&format!("rtmp://127.0.0.1:{}/live/{}", rtmp_port, key),
		])
		.stdout(std::process::Stdio::inherit())
		.stderr(std::process::Stdio::inherit())
		.spawn()
		.expect("failed to execute ffmpeg")
}

fn stream_with_ffmpeg_tls(rtmp_port: u16, file: &str, tls_dir: &Path, key: &str) -> tokio::process::Child {
	let video_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../assets");

	Command::new("ffmpeg")
		.args([
			"-re",
			"-i",
			video_dir.join(file).to_str().expect("failed to get path"),
			"-c",
			"copy",
			"-tls_verify",
			"1",
			"-ca_file",
			tls_dir.join("ca.crt").to_str().unwrap(),
			"-cert_file",
			tls_dir.join("client.crt").to_str().unwrap(),
			"-key_file",
			tls_dir.join("client.key").to_str().unwrap(),
			"-f",
			"flv",
			&format!("rtmps://localhost:{}/live/{}", rtmp_port, key),
		])
		.stdout(std::process::Stdio::inherit())
		.stderr(std::process::Stdio::inherit())
		.spawn()
		.expect("failed to execute ffmpeg")
}

fn spawn_ffprobe() -> tokio::process::Child {
	Command::new("ffprobe")
		.arg("-v")
		.arg("error")
		.arg("-fpsprobesize")
		.arg("20000")
		.arg("-show_format")
		.arg("-show_streams")
		.arg("-print_format")
		.arg("json")
		.arg("-")
		.stdin(Stdio::piped())
		.stdout(Stdio::piped())
		.stderr(Stdio::inherit())
		.spawn()
		.unwrap()
}

struct Watcher {
	pub send: mpsc::Sender<IngestWatchRequest>,
	pub recv: tonic::Streaming<IngestWatchResponse>,
}

impl Watcher {
	async fn new(request_id: Ulid, advertise_addr: String) -> Self {
		let (send, rx) = mpsc::channel(10);

		send.send(IngestWatchRequest {
			message: Some(ingest_watch_request::Message::Open(ingest_watch_request::Open {
				request_id: Some(request_id.into()),
			})),
		})
		.await
		.unwrap();

		tracing::info!("connecting to ingest server at {}", advertise_addr);

		let channel = scuffle_utilsgrpc::make_channel(vec![advertise_addr], Duration::from_secs(30), None).unwrap();

		let mut client = IngestClient::new(channel);

		let recv = client
			.watch(tokio_stream::wrappers::ReceiverStream::new(rx))
			.await
			.unwrap()
			.into_inner();

		Self { send, recv }
	}

	async fn recv(&mut self) -> IngestWatchResponse {
		tokio::time::timeout(Duration::from_secs(2), self.recv.message())
			.await
			.expect("failed to receive event")
			.expect("failed to receive event")
			.expect("failed to receive event")
	}
}

struct TestState {
	pub rtmp_port: u16,
	pub org_id: Ulid,
	pub room_id: Ulid,
	pub global: Arc<GlobalState>,
	pub handler: scuffle_utils::context::Handler,
	pub transcoder_requests: Pin<Box<dyn futures::Stream<Item = TranscoderRequestTask>>>,
	pub events: Pin<Box<dyn futures::Stream<Item = Event>>>,
	pub ingest_handle: JoinHandle<anyhow::Result<()>>,
	pub grpc_handle: JoinHandle<Result<(), tonic::transport::Error>>,
}

impl TestState {
	async fn setup() -> Self {
		Self::setup_new(None).await
	}

	async fn setup_with_tls(tls_dir: &Path) -> Self {
		Self::setup_new(Some(TlsConfig {
			cert: tls_dir.join("server.crt").to_str().unwrap().to_string(),
			ca_cert: Some(tls_dir.join("ca.crt").to_str().unwrap().to_string()),
			key: tls_dir.join("server.key").to_str().unwrap().to_string(),
			domain: Some("localhost".to_string()),
		}))
		.await
	}

	async fn setup_new(tls: Option<TlsConfig>) -> Self {
		let grpc_port = portpicker::pick_unused_port().unwrap();
		let rtmp_port = portpicker::pick_unused_port().unwrap();

		let (global, handler) = mock_global_state(IngestConfig {
			events_stream_name: Ulid::new().to_string(),
			transcoder_request_subject: Uuid::new_v4().to_string(),
			bitrate_update_interval: Duration::from_secs(1),
			grpc_advertise_address: format!("127.0.0.1:{grpc_port}"),
			rtmp: RtmpConfig {
				bind_address: format!("127.0.0.1:{rtmp_port}").parse().unwrap(),
				tls,
			},
			..Default::default()
		})
		.await;

		let grpc = crate::grpc::add_routes(&global, tonic::transport::Server::builder().add_routes(Default::default()));

		let ingest_handle = tokio::spawn(crate::ingest::run(global.clone()));
		let grpc_handle = {
			let global = global.clone();
			tokio::spawn(async move {
				grpc.serve_with_shutdown(([127, 0, 0, 1], grpc_port).into(), async {
					global.ctx().done().await;
				})
				.await
			})
		};

		let transcoder_requests = {
			let global = global.clone();
			let mut stream = global
				.nats()
				.subscribe(global.config::<IngestConfig>().transcoder_request_subject.clone())
				.await
				.unwrap();

			stream!({
				while let Ok(Some(message)) = stream.next().context(global.ctx()).await {
					yield TranscoderRequestTask::decode(message.payload).unwrap();
				}
			})
		};

		let org_id = Ulid::new();

		let events = {
			let global = global.clone();
			let mut stream = global
				.nats()
				.subscribe(event_subject(&global.config().events_stream_name, org_id, Target::Room))
				.await
				.unwrap();

			stream!({
				while let Ok(Some(message)) = stream.next().context(global.ctx()).await {
					yield Event::decode(message.payload).unwrap();
				}
			})
		};

		scuffle_utils::database::query("INSERT INTO organizations (id, name) VALUES ($1, $2)")
			.bind(org_id)
			.bind("test")
			.build()
			.execute(global.db())
			.await
			.unwrap();

		let room_id = Ulid::new();

		scuffle_utils::database::query("INSERT INTO rooms (organization_id, id, stream_key) VALUES ($1, $2, $3)")
			.bind(org_id)
			.bind(room_id)
			.bind(room_id.to_string())
			.build()
			.execute(global.db())
			.await
			.unwrap();

		Self {
			org_id,
			room_id,
			rtmp_port,
			global,
			handler,
			events: Box::pin(events),
			transcoder_requests: Box::pin(transcoder_requests),
			ingest_handle,
			grpc_handle,
		}
	}

	async fn transcoder_request(&mut self) -> TranscoderRequestTask {
		tokio::time::timeout(Duration::from_secs(2), self.transcoder_requests.next())
			.await
			.expect("failed to receive event")
			.expect("failed to receive event")
	}

	async fn organization_event(&mut self) -> Event {
		tokio::time::timeout(Duration::from_secs(2), self.events.next())
			.await
			.expect("failed to receive event")
			.expect("failed to receive event")
	}

	fn finish(self) -> impl futures::Future<Output = ()> {
		let handler = self.handler;
		let ingest_handle = self.ingest_handle;
		let grpc_handle = self.grpc_handle;
		async move {
			handler.cancel().await;
			assert!(ingest_handle.is_finished());
			assert!(grpc_handle.is_finished());
		}
	}
}

#[tokio::test]
async fn test_ingest_stream() {
	let mut state = TestState::setup().await;
	let mut ffmpeg = stream_with_ffmpeg(
		state.rtmp_port,
		"avc_aac_keyframes.mp4",
		&generate_key(state.org_id, state.room_id),
	);

	let update = state.organization_event().await;
	assert!(update.timestamp.sub(chrono::Utc::now().timestamp_millis()) < 1000);
	assert!(!update.event_id.into_ulid().is_nil(), "event_id is nil");
	match update.event {
		Some(event::Event::Room(room)) => {
			assert_eq!(room.room_id.into_ulid(), state.room_id);
			match room.event {
				Some(event::room::Event::Connected(live)) => {
					assert!(!live.connection_id.into_ulid().is_nil());
				}
				_ => panic!("unexpected event"),
			}
		}
		_ => panic!("unexpected event"),
	}

	let room: video_common::database::Room =
		scuffle_utils::database::query("SELECT * FROM rooms WHERE organization_id = $1 AND id = $2")
			.bind(state.org_id)
			.bind(state.room_id)
			.build_query_as()
			.fetch_one(state.global.db())
			.await
			.unwrap();

	assert!(room.last_live_at.is_some());
	assert!(room.last_disconnected_at.is_none());
	assert!(room.video_input.is_some());
	assert!(room.audio_input.is_some());

	let video_input = room.video_input.unwrap();
	let audio_input = room.audio_input.unwrap();

	assert_eq!(video_input.rendition(), Rendition::VideoSource);
	assert_eq!(video_input.codec, "avc1.64001f");
	assert_eq!(video_input.width, 480);
	assert_eq!(video_input.height, 852);
	assert_eq!(video_input.fps, 30);
	assert_eq!(video_input.bitrate, 1276158);

	assert_eq!(audio_input.rendition(), Rendition::AudioSource);
	assert_eq!(audio_input.codec, "mp4a.40.2");
	assert_eq!(audio_input.sample_rate, 44100);
	assert_eq!(audio_input.channels, 2);
	assert_eq!(audio_input.bitrate, 69568);

	assert_eq!(room.status, video_common::database::RoomStatus::WaitingForTranscoder);

	let msg = state.transcoder_request().await;
	assert!(!msg.request_id.into_ulid().is_nil());
	assert!(!msg.room_id.into_ulid().is_nil());
	assert!(!msg.connection_id.into_ulid().is_nil());
	assert!(!msg.organization_id.into_ulid().is_nil());
	assert!(!msg.grpc_endpoint.is_empty());

	// We should now be able to join the stream
	let mut watcher = Watcher::new(msg.request_id.into_ulid(), msg.grpc_endpoint).await;

	match watcher.recv().await.message {
		Some(ingest_watch_response::Message::Media(media)) => {
			assert_eq!(media.r#type(), ingest_watch_response::media::Type::Init);
			assert!(!media.data.is_empty());
		}
		_ => panic!("unexpected event"),
	}

	match watcher.recv().await.message {
		Some(ingest_watch_response::Message::Ready(_)) => {}
		_ => panic!("unexpected event"),
	}

	match watcher.recv().await.message {
		Some(ingest_watch_response::Message::Media(media)) => {
			assert!(!media.data.is_empty());
		}
		_ => panic!("unexpected event"),
	}

	watcher
		.send
		.send(IngestWatchRequest {
			message: Some(ingest_watch_request::Message::Shutdown(
				ingest_watch_request::Shutdown::Request as i32,
			)),
		})
		.await
		.unwrap();

	// It should now create a new transcoder to handle the stream
	let msg = state.transcoder_request().await;
	assert!(!msg.request_id.into_ulid().is_nil());
	assert!(!msg.room_id.into_ulid().is_nil());
	assert!(!msg.connection_id.into_ulid().is_nil());
	assert!(!msg.organization_id.into_ulid().is_nil());
	assert!(!msg.grpc_endpoint.is_empty());

	// We should now be able to join the stream
	let mut new_watcher = Watcher::new(msg.request_id.into_ulid(), msg.grpc_endpoint).await;

	let mut got_shutting_down = false;
	while let Ok(Some(msg)) = watcher.recv.message().await {
		match msg.message.unwrap() {
			ingest_watch_response::Message::Media(media) => {
				assert!(!media.data.is_empty());
			}
			ingest_watch_response::Message::Ready(_) => {
				panic!("unexpected ready");
			}
			ingest_watch_response::Message::Shutdown(s) => {
				assert_eq!(s, ingest_watch_response::Shutdown::Transcoder as i32);
				got_shutting_down = true;
				break;
			}
		}
	}

	watcher
		.send
		.send(IngestWatchRequest {
			message: Some(ingest_watch_request::Message::Shutdown(
				ingest_watch_request::Shutdown::Complete as i32,
			)),
		})
		.await
		.unwrap();

	assert!(got_shutting_down);

	tokio::time::sleep(Duration::from_millis(200)).await;

	let update = state.organization_event().await;
	assert!(update.timestamp.sub(chrono::Utc::now().timestamp_millis()) < 1000);
	assert!(!update.event_id.into_ulid().is_nil(), "event_id is nil");
	match update.event {
		Some(event::Event::Room(room)) => {
			assert_eq!(room.room_id.into_ulid(), state.room_id);
			match room.event {
				Some(event::room::Event::TranscoderDisconnected(disconnected)) => {
					assert!(!disconnected.connection_id.into_ulid().is_nil());
					assert!(disconnected.clean);
				}
				_ => panic!("unexpected event: {:#?}", room),
			}
		}
		_ => panic!("unexpected event"),
	}

	match new_watcher.recv().await.message {
		Some(ingest_watch_response::Message::Media(media)) => {
			assert_eq!(media.r#type(), ingest_watch_response::media::Type::Init);
			assert!(!media.data.is_empty());
		}
		_ => panic!("unexpected event"),
	}

	let mut got_ready = false;

	while let Ok(Some(msg)) = new_watcher.recv.message().await {
		match msg.message.unwrap() {
			ingest_watch_response::Message::Media(media) => {
				assert!(!media.data.is_empty());
			}
			ingest_watch_response::Message::Ready(_) => {
				got_ready = true;
				break;
			}
			ingest_watch_response::Message::Shutdown(_) => {
				panic!("unexpected shutdown");
			}
		}
	}

	assert!(got_ready);

	if let Ok(Some(msg)) = watcher.recv.message().await {
		panic!("unexpected message: {:?}", msg);
	}

	// Assert that no messages with keyframes made it to the old channel

	ffmpeg.kill().await.unwrap();

	tokio::time::sleep(Duration::from_millis(200)).await;

	let update = state.organization_event().await;
	assert!(update.timestamp.sub(chrono::Utc::now().timestamp_millis()) < 1000);
	assert!(!update.event_id.into_ulid().is_nil(), "event_id is nil");
	match update.event {
		Some(event::Event::Room(room)) => {
			assert_eq!(room.room_id.into_ulid(), state.room_id);
			match room.event {
				Some(event::room::Event::Disconnected(disconnected)) => {
					assert!(!disconnected.connection_id.into_ulid().is_nil());
					assert!(!disconnected.clean);
					assert!(disconnected.cause.is_none());
				}
				_ => panic!("unexpected event: {:#?}", room),
			}
		}
		_ => panic!("unexpected event"),
	}

	tracing::info!("waiting for transcoder to exit");

	let room: Room = scuffle_utils::database::query("SELECT * FROM rooms WHERE organization_id = $1 AND id = $2")
		.bind(state.org_id)
		.bind(state.room_id)
		.build_query_as()
		.fetch_one(state.global.db())
		.await
		.unwrap();

	assert_eq!(room.status, video_common::database::RoomStatus::Offline);
	assert!(room.last_disconnected_at.is_some());
	assert!(room.last_live_at.is_some());
	assert!(room.video_input.is_none());
	assert!(room.audio_input.is_none());

	state.finish().await;
}

#[tokio::test]
async fn test_ingest_stream_transcoder_disconnect() {
	let mut state = TestState::setup().await;
	let mut ffmpeg = stream_with_ffmpeg(
		state.rtmp_port,
		"avc_aac_keyframes.mp4",
		&generate_key(state.org_id, state.room_id),
	);

	let update = state.organization_event().await;
	assert!(update.timestamp.sub(chrono::Utc::now().timestamp_millis()) < 1000);
	assert!(!update.event_id.into_ulid().is_nil(), "event_id is nil");
	match update.event {
		Some(event::Event::Room(room)) => {
			assert_eq!(room.room_id.into_ulid(), state.room_id);
			match room.event {
				Some(event::room::Event::Connected(live)) => {
					assert!(!live.connection_id.into_ulid().is_nil());
				}
				_ => panic!("unexpected event"),
			}
		}
		_ => panic!("unexpected event"),
	}

	let msg = state.transcoder_request().await;
	assert!(!msg.request_id.into_ulid().is_nil());
	assert!(!msg.room_id.into_ulid().is_nil());
	assert!(!msg.connection_id.into_ulid().is_nil());
	assert!(!msg.organization_id.into_ulid().is_nil());
	assert!(!msg.grpc_endpoint.is_empty());

	// We should now be able to join the stream
	let mut watcher = Watcher::new(msg.request_id.into_ulid(), msg.grpc_endpoint).await;

	match watcher.recv().await.message {
		Some(ingest_watch_response::Message::Media(media)) => {
			assert_eq!(media.r#type(), ingest_watch_response::media::Type::Init);
			assert!(!media.data.is_empty());
		}
		_ => panic!("unexpected event"),
	}

	match watcher.recv().await.message {
		Some(ingest_watch_response::Message::Ready(_)) => {}
		_ => panic!("unexpected event"),
	}

	match watcher.recv().await.message {
		Some(ingest_watch_response::Message::Media(media)) => {
			assert!(!media.data.is_empty());
		}
		_ => panic!("unexpected event"),
	}

	// Force disconnect the transcoder
	drop(watcher);

	tokio::time::sleep(Duration::from_millis(200)).await;

	let update = state.organization_event().await;
	assert!(update.timestamp.sub(chrono::Utc::now().timestamp_millis()) < 1000);
	assert!(!update.event_id.into_ulid().is_nil(), "event_id is nil");
	match update.event {
		Some(event::Event::Room(room)) => {
			assert_eq!(room.room_id.into_ulid(), state.room_id);
			match room.event {
				Some(event::room::Event::TranscoderDisconnected(disconnected)) => {
					assert!(!disconnected.connection_id.into_ulid().is_nil());
					assert!(!disconnected.clean);
				}
				_ => panic!("unexpected event: {:#?}", room),
			}
		}
		_ => panic!("unexpected event"),
	}

	let msg = state.transcoder_request().await;
	assert!(!msg.request_id.into_ulid().is_nil());
	assert!(!msg.room_id.into_ulid().is_nil());
	assert!(!msg.connection_id.into_ulid().is_nil());
	assert!(!msg.organization_id.into_ulid().is_nil());
	assert!(!msg.grpc_endpoint.is_empty());

	let mut watcher = Watcher::new(msg.request_id.into_ulid(), msg.grpc_endpoint).await;

	match watcher.recv().await.message {
		Some(ingest_watch_response::Message::Media(media)) => {
			assert_eq!(media.r#type(), ingest_watch_response::media::Type::Init);
			assert!(!media.data.is_empty());
		}
		_ => panic!("unexpected event"),
	}

	match watcher.recv().await.message {
		Some(ingest_watch_response::Message::Ready(_)) => {}
		_ => panic!("unexpected event"),
	}

	match watcher.recv().await.message {
		Some(ingest_watch_response::Message::Media(media)) => {
			assert!(!media.data.is_empty());
		}
		r => panic!("unexpected event: {:?}", r),
	}

	ffmpeg.kill().await.unwrap();

	let update = state.organization_event().await;
	assert!(update.timestamp.sub(chrono::Utc::now().timestamp_millis()) < 1000);
	assert!(!update.event_id.into_ulid().is_nil(), "event_id is nil");
	match update.event {
		Some(event::Event::Room(room)) => {
			assert_eq!(room.room_id.into_ulid(), state.room_id);
			match room.event {
				Some(event::room::Event::Disconnected(disconnected)) => {
					assert!(!disconnected.connection_id.into_ulid().is_nil());
					assert!(!disconnected.clean);
					assert!(disconnected.cause.is_none())
				}
				_ => panic!("unexpected event"),
			}
		}
		_ => panic!("unexpected event"),
	}

	state.finish().await;
}

#[tokio::test]
async fn test_ingest_stream_shutdown() {
	let mut state = TestState::setup().await;
	let mut ffmpeg = stream_with_ffmpeg(
		state.rtmp_port,
		"avc_aac_keyframes.mp4",
		&generate_key(state.org_id, state.room_id),
	);

	let update = state.organization_event().await;
	assert!(update.timestamp.sub(chrono::Utc::now().timestamp_millis()) < 1000);
	assert!(!update.event_id.into_ulid().is_nil(), "event_id is nil");
	let connection_id: Ulid;
	match update.event {
		Some(event::Event::Room(room)) => {
			assert_eq!(room.room_id.into_ulid(), state.room_id);
			match room.event {
				Some(event::room::Event::Connected(live)) => {
					assert!(!live.connection_id.into_ulid().is_nil());
					connection_id = live.connection_id.into_ulid();
				}
				_ => panic!("unexpected event"),
			}
		}
		_ => panic!("unexpected event"),
	}

	state
		.global
		.nats()
		.publish(keys::ingest_disconnect(connection_id), Bytes::new())
		.await
		.unwrap();

	tracing::info!("waiting for transcoder to exit");

	assert!(ffmpeg.wait().timeout(Duration::from_secs(1)).await.is_ok());

	let update = state.organization_event().await;
	assert!(update.timestamp.sub(chrono::Utc::now().timestamp_millis()) < 1000);
	assert!(!update.event_id.into_ulid().is_nil(), "event_id is nil");
	match update.event {
		Some(event::Event::Room(room)) => {
			assert_eq!(room.room_id.into_ulid(), state.room_id);
			match room.event {
				Some(event::room::Event::Disconnected(disconnected)) => {
					assert!(!disconnected.connection_id.into_ulid().is_nil());
					assert!(disconnected.clean);
					assert_eq!(disconnected.cause(), "I14: Disconnect requested");
				}
				_ => panic!("unexpected event"),
			}
		}
		_ => panic!("unexpected event"),
	}

	let room: Room = scuffle_utils::database::query("SELECT * FROM rooms WHERE organization_id = $1 AND id = $2")
		.bind(state.org_id)
		.bind(state.room_id)
		.build_query_as()
		.fetch_one(state.global.db())
		.await
		.unwrap();

	assert_eq!(room.status, video_common::database::RoomStatus::Offline);
	assert!(room.last_disconnected_at.is_some());
	assert!(room.last_live_at.is_some());
	assert!(room.active_ingest_connection_id.is_none());

	state.finish().await;
}

#[tokio::test]
async fn test_ingest_stream_transcoder_full() {
	let mut state = TestState::setup().await;
	let mut ffmpeg = stream_with_ffmpeg(
		state.rtmp_port,
		"avc_aac_large.mp4",
		&generate_key(state.org_id, state.room_id),
	);

	let update = state.organization_event().await;
	assert!(update.timestamp.sub(chrono::Utc::now().timestamp_millis()) < 1000);
	assert!(!update.event_id.into_ulid().is_nil(), "event_id is nil");
	match update.event {
		Some(event::Event::Room(room)) => {
			assert_eq!(room.room_id.into_ulid(), state.room_id);
			match room.event {
				Some(event::room::Event::Connected(live)) => {
					assert!(!live.connection_id.into_ulid().is_nil());
				}
				_ => panic!("unexpected event"),
			}
		}
		_ => panic!("unexpected event"),
	}

	let room: Room = scuffle_utils::database::query("SELECT * FROM rooms WHERE organization_id = $1 AND id = $2")
		.bind(state.org_id)
		.bind(state.room_id)
		.build_query_as()
		.fetch_one(state.global.db())
		.await
		.unwrap();

	assert_eq!(room.status, video_common::database::RoomStatus::WaitingForTranscoder);
	assert!(room.last_disconnected_at.is_none());
	assert!(room.last_live_at.is_some());
	assert!(room.active_ingest_connection_id.is_some());
	assert!(room.video_input.is_some());
	assert!(room.audio_input.is_some());

	let video_input = room.video_input.unwrap();
	let audio_input = room.audio_input.unwrap();
	assert_eq!(video_input.codec, "avc1.640034");
	assert_eq!(audio_input.codec, "mp4a.40.2");
	assert_eq!(video_input.width, 3840);
	assert_eq!(video_input.height, 2160);
	assert_eq!(video_input.fps, 60);
	assert_eq!(audio_input.sample_rate, 48000);
	assert_eq!(audio_input.channels, 2);
	assert_eq!(video_input.bitrate, 1740285);
	assert_eq!(audio_input.bitrate, 140304);

	let msg = state.transcoder_request().await;
	assert_eq!(
		msg.connection_id.into_ulid(),
		room.active_ingest_connection_id.unwrap().0.into(),
	);
	assert!(!msg.request_id.into_ulid().is_nil());
	assert_eq!(msg.organization_id.into_ulid(), state.org_id);
	assert_eq!(msg.room_id.into_ulid(), state.room_id);

	// We should now be able to join the stream
	let mut watcher = Watcher::new(msg.request_id.into_ulid(), msg.grpc_endpoint).await;

	match watcher.recv().await.message {
		Some(ingest_watch_response::Message::Media(media)) => {
			assert_eq!(media.r#type(), ingest_watch_response::media::Type::Init);
			assert!(!media.data.is_empty());
		}
		_ => panic!("unexpected event"),
	}

	match watcher.recv().await.message {
		Some(ingest_watch_response::Message::Ready(_)) => {}
		_ => panic!("unexpected event"),
	}

	match watcher.recv().await.message {
		Some(ingest_watch_response::Message::Media(media)) => {
			assert!(!media.data.is_empty());
		}
		_ => panic!("unexpected event"),
	}

	let mut got_shutting_down = false;

	while let Ok(Some(msg)) = watcher.recv.message().await {
		match msg.message.unwrap() {
			ingest_watch_response::Message::Media(media) => {
				assert!(!media.data.is_empty());
			}
			ingest_watch_response::Message::Ready(_) => {
				panic!("unexpected ready");
			}
			ingest_watch_response::Message::Shutdown(_) => {
				got_shutting_down = true;
				break;
			}
		}
	}

	assert!(got_shutting_down);

	tokio::time::sleep(Duration::from_millis(200)).await;

	assert!(ffmpeg.try_wait().is_ok());

	let update = state.organization_event().await;
	assert!(update.timestamp.sub(chrono::Utc::now().timestamp_millis()) < 1000);
	assert!(!update.event_id.into_ulid().is_nil(), "event_id is nil");
	match update.event {
		Some(event::Event::Room(room)) => {
			assert_eq!(room.room_id.into_ulid(), state.room_id);
			match room.event {
				Some(event::room::Event::Disconnected(disconnected)) => {
					assert!(!disconnected.connection_id.into_ulid().is_nil());
					assert!(disconnected.clean);
					assert!(disconnected.cause.is_none());
				}
				_ => panic!("unexpected event"),
			}
		}
		_ => panic!("unexpected event"),
	}

	state.finish().await;
}

#[tokio::test]
async fn test_ingest_stream_reject() {
	let state = TestState::setup().await;

	let bad_keys = vec![
		"bad_key".into(),
		"live_bad_key".into(),
		generate_key(state.org_id, state.org_id),
		generate_key(state.room_id, state.room_id),
	];

	for bad_key in bad_keys {
		let mut ffmpeg = stream_with_ffmpeg(state.rtmp_port, "avc_aac_large.mp4", &bad_key);

		tokio::time::sleep(Duration::from_millis(200)).await;

		assert!(ffmpeg.try_wait().is_ok());
	}

	state.finish().await;
}

async fn test_ingest_stream_transcoder_full_tls(tls_dir: PathBuf) {
	let mut state = TestState::setup_with_tls(&tls_dir).await;
	let mut ffmpeg = stream_with_ffmpeg_tls(
		state.rtmp_port,
		"avc_aac_large.mp4",
		&tls_dir,
		&generate_key(state.org_id, state.room_id),
	);

	let update = state.organization_event().await;
	assert!(update.timestamp.sub(chrono::Utc::now().timestamp_millis()) < 1000);
	assert!(!update.event_id.into_ulid().is_nil(), "event_id is nil");
	match update.event {
		Some(event::Event::Room(room)) => {
			assert_eq!(room.room_id.into_ulid(), state.room_id);
			match room.event {
				Some(event::room::Event::Connected(live)) => {
					assert!(!live.connection_id.into_ulid().is_nil());
				}
				_ => panic!("unexpected event"),
			}
		}
		_ => panic!("unexpected event"),
	}

	let msg = state.transcoder_request().await;
	assert!(!msg.request_id.into_ulid().is_nil());
	assert!(!msg.room_id.into_ulid().is_nil());
	assert!(!msg.connection_id.into_ulid().is_nil());
	assert!(!msg.organization_id.into_ulid().is_nil());
	assert!(!msg.grpc_endpoint.is_empty());

	// We should now be able to join the stream
	let mut watcher = Watcher::new(msg.request_id.into_ulid(), msg.grpc_endpoint).await;

	match watcher.recv().await.message {
		Some(ingest_watch_response::Message::Media(media)) => {
			assert_eq!(media.r#type(), ingest_watch_response::media::Type::Init);
			assert!(!media.data.is_empty());
		}
		_ => panic!("unexpected event"),
	}

	match watcher.recv().await.message {
		Some(ingest_watch_response::Message::Ready(_)) => {}
		_ => panic!("unexpected event"),
	}

	match watcher.recv().await.message {
		Some(ingest_watch_response::Message::Media(media)) => {
			assert!(!media.data.is_empty());
		}
		_ => panic!("unexpected event"),
	}

	let mut got_shutting_down = false;

	while let Ok(Some(msg)) = watcher.recv.message().await {
		match msg.message.unwrap() {
			ingest_watch_response::Message::Media(media) => {
				assert!(!media.data.is_empty());
			}
			ingest_watch_response::Message::Ready(_) => {
				panic!("unexpected ready");
			}
			ingest_watch_response::Message::Shutdown(_) => {
				got_shutting_down = true;
				break;
			}
		}
	}

	assert!(got_shutting_down);

	tokio::time::sleep(Duration::from_millis(200)).await;

	assert!(ffmpeg.try_wait().is_ok());

	let update = state.organization_event().await;
	assert!(update.timestamp.sub(chrono::Utc::now().timestamp_millis()) < 1000);
	assert!(!update.event_id.into_ulid().is_nil(), "event_id is nil");
	match update.event {
		Some(event::Event::Room(room)) => {
			assert_eq!(room.room_id.into_ulid(), state.room_id);
			match room.event {
				Some(event::room::Event::Disconnected(disconnected)) => {
					assert!(!disconnected.connection_id.into_ulid().is_nil());
					assert!(disconnected.clean);
					assert!(disconnected.cause.is_none());
				}
				_ => panic!("unexpected event"),
			}
		}
		_ => panic!("unexpected event"),
	}

	state.finish().await;
}

#[tokio::test]
async fn test_ingest_stream_transcoder_full_tls_rsa() {
	test_ingest_stream_transcoder_full_tls(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/tests/certs/rsa")).await;
}

#[tokio::test]
async fn test_ingest_stream_transcoder_full_tls_ec() {
	test_ingest_stream_transcoder_full_tls(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/tests/certs/ec")).await;
}

#[tokio::test]
async fn test_ingest_stream_transcoder_probe() {
	let mut state = TestState::setup().await;
	let mut ffmpeg = stream_with_ffmpeg(
		state.rtmp_port,
		"avc_aac_keyframes.mp4",
		&generate_key(state.org_id, state.room_id),
	);

	let update = state.organization_event().await;
	assert!(update.timestamp.sub(chrono::Utc::now().timestamp_millis()) < 1000);
	assert!(!update.event_id.into_ulid().is_nil(), "event_id is nil");
	match update.event {
		Some(event::Event::Room(room)) => {
			assert_eq!(room.room_id.into_ulid(), state.room_id);
			match room.event {
				Some(event::room::Event::Connected(live)) => {
					assert!(!live.connection_id.into_ulid().is_nil());
				}
				_ => panic!("unexpected event"),
			}
		}
		_ => panic!("unexpected event"),
	}

	let mut ffprobe = spawn_ffprobe();
	let writer = ffprobe.stdin.as_mut().unwrap();

	let msg = state.transcoder_request().await;
	assert!(!msg.request_id.into_ulid().is_nil());
	assert_eq!(msg.organization_id.into_ulid(), state.org_id);
	assert_eq!(msg.room_id.into_ulid(), state.room_id);

	// We should now be able to join the stream
	let mut watcher = Watcher::new(msg.request_id.into_ulid(), msg.grpc_endpoint).await;

	match watcher.recv().await.message {
		Some(ingest_watch_response::Message::Media(media)) => {
			assert_eq!(media.r#type(), ingest_watch_response::media::Type::Init);
			assert!(!media.data.is_empty());
			writer.write_all(&media.data).await.unwrap();
		}
		_ => panic!("unexpected event"),
	}

	match watcher.recv().await.message {
		Some(ingest_watch_response::Message::Ready(_)) => {}
		_ => panic!("unexpected event"),
	}

	match watcher.recv().await.message {
		Some(ingest_watch_response::Message::Media(media)) => {
			assert!(!media.data.is_empty());
			writer.write_all(&media.data).await.unwrap();
		}
		_ => panic!("unexpected event"),
	}

	// Finish the stream
	let mut got_shutting_down = false;
	while let Ok(Some(msg)) = watcher.recv.message().await {
		match msg.message.unwrap() {
			ingest_watch_response::Message::Media(media) => {
				assert!(!media.data.is_empty());
				writer.write_all(&media.data).await.unwrap();
			}
			ingest_watch_response::Message::Ready(_) => {
				panic!("unexpected ready");
			}
			ingest_watch_response::Message::Shutdown(_) => {
				got_shutting_down = true;
				break;
			}
		}
	}

	assert!(got_shutting_down);

	tokio::time::sleep(Duration::from_millis(200)).await;

	assert!(ffmpeg.try_wait().is_ok());

	let output = ffprobe.wait_with_output().await.unwrap();
	assert!(output.status.success());

	let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

	{
		let video_stream = &json["streams"][0];
		assert_eq!(video_stream["codec_type"], "video");
		assert_eq!(video_stream["codec_name"], "h264");
		assert_eq!(video_stream["width"], 480);
		assert_eq!(video_stream["height"], 852);
		assert_eq!(video_stream["r_frame_rate"], "30/1");
		assert_eq!(video_stream["avg_frame_rate"], "30/1");
		assert_eq!(video_stream["time_base"], "1/30000");
		assert_eq!(video_stream["codec_tag"], "0x31637661");
		assert_eq!(video_stream["codec_tag_string"], "avc1");
		assert_eq!(video_stream["profile"], "High");
		assert_eq!(video_stream["level"], 31);
		assert_eq!(video_stream["refs"], 1);
		assert_eq!(video_stream["is_avc"], "true");

		let audio_stream = &json["streams"][1];
		assert_eq!(audio_stream["codec_type"], "audio");
		assert_eq!(audio_stream["codec_name"], "aac");
		assert_eq!(audio_stream["sample_rate"], "44100");
		assert_eq!(audio_stream["channels"], 1);
		assert_eq!(audio_stream["channel_layout"], "mono");
		assert_eq!(audio_stream["r_frame_rate"], "0/0");
		assert_eq!(audio_stream["avg_frame_rate"], "0/0");
		assert_eq!(audio_stream["time_base"], "1/44100");
		assert_eq!(audio_stream["codec_tag"], "0x6134706d");
		assert_eq!(audio_stream["codec_tag_string"], "mp4a");
		assert_eq!(audio_stream["profile"], "LC");
	}

	state.finish().await;
}

#[tokio::test]
async fn test_ingest_stream_transcoder_probe_reconnect() {
	let mut state = TestState::setup().await;
	let mut ffmpeg = stream_with_ffmpeg(
		state.rtmp_port,
		"avc_aac_keyframes.mp4",
		&generate_key(state.org_id, state.room_id),
	);

	let mut ffprobe = spawn_ffprobe();
	let writer = ffprobe.stdin.as_mut().unwrap();

	let msg = state.transcoder_request().await;

	let mut watcher = Watcher::new(msg.request_id.into_ulid(), msg.grpc_endpoint).await;

	match watcher.recv().await.message {
		Some(ingest_watch_response::Message::Media(media)) => {
			assert_eq!(media.r#type(), ingest_watch_response::media::Type::Init);
			assert!(!media.data.is_empty());
			writer.write_all(&media.data).await.unwrap();
		}
		_ => panic!("unexpected event"),
	}

	match watcher.recv().await.message {
		Some(ingest_watch_response::Message::Ready(_)) => {}
		_ => panic!("unexpected event"),
	}

	let mut i = 0;
	while let Ok(Some(msg)) = watcher.recv.message().await {
		match msg.message.unwrap() {
			ingest_watch_response::Message::Media(media) => {
				assert!(!media.data.is_empty());
				writer.write_all(&media.data).await.unwrap();
			}
			_ => panic!("unexpected event"),
		}

		i += 1;

		if i == 10 {
			break;
		}
	}

	watcher
		.send
		.send(IngestWatchRequest {
			message: Some(ingest_watch_request::Message::Shutdown(
				ingest_watch_request::Shutdown::Request.into(),
			)),
		})
		.await
		.unwrap();

	let output = ffprobe.wait_with_output().await.unwrap();
	assert!(output.status.success());

	let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

	{
		let video_stream = &json["streams"][0];
		assert_eq!(video_stream["codec_type"], "video");
		assert_eq!(video_stream["codec_name"], "h264");
		assert_eq!(video_stream["width"], 480);
		assert_eq!(video_stream["height"], 852);
		assert_eq!(video_stream["r_frame_rate"], "30/1");
		assert_eq!(video_stream["avg_frame_rate"], "30/1");
		assert_eq!(video_stream["time_base"], "1/30000");
		assert_eq!(video_stream["codec_tag"], "0x31637661");
		assert_eq!(video_stream["codec_tag_string"], "avc1");
		assert_eq!(video_stream["profile"], "High");
		assert_eq!(video_stream["level"], 31);
		assert_eq!(video_stream["refs"], 1);
		assert_eq!(video_stream["is_avc"], "true");

		let audio_stream = &json["streams"][1];
		assert_eq!(audio_stream["codec_type"], "audio");
		assert_eq!(audio_stream["codec_name"], "aac");
		assert_eq!(audio_stream["sample_rate"], "44100");
		assert_eq!(audio_stream["channels"], 1);
		assert_eq!(audio_stream["channel_layout"], "mono");
		assert_eq!(audio_stream["r_frame_rate"], "0/0");
		assert_eq!(audio_stream["avg_frame_rate"], "0/0");
		assert_eq!(audio_stream["time_base"], "1/44100");
		assert_eq!(audio_stream["codec_tag"], "0x6134706d");
		assert_eq!(audio_stream["codec_tag_string"], "mp4a");
		assert_eq!(audio_stream["profile"], "LC");
	}

	let msg = state.transcoder_request().await;

	let mut new_watcher = Watcher::new(msg.request_id.into_ulid(), msg.grpc_endpoint).await;

	let mut got_shutting_down = false;
	while let Ok(Some(msg)) = watcher.recv.message().await {
		match msg.message.unwrap() {
			ingest_watch_response::Message::Media(media) => {
				assert!(!media.data.is_empty());
			}
			ingest_watch_response::Message::Ready(_) => {
				panic!("unexpected ready");
			}
			ingest_watch_response::Message::Shutdown(_) => {
				got_shutting_down = true;
				break;
			}
		}
	}

	assert!(got_shutting_down);

	watcher
		.send
		.send(IngestWatchRequest {
			message: Some(ingest_watch_request::Message::Shutdown(
				ingest_watch_request::Shutdown::Complete.into(),
			)),
		})
		.await
		.unwrap();

	let mut ffprobe = spawn_ffprobe();
	let writer = ffprobe.stdin.as_mut().unwrap();

	match new_watcher.recv().await.message {
		Some(ingest_watch_response::Message::Media(media)) => {
			assert_eq!(media.r#type(), ingest_watch_response::media::Type::Init);
			assert!(!media.data.is_empty());
			writer.write_all(&media.data).await.unwrap();
		}
		_ => panic!("unexpected event"),
	}

	let mut got_ready = false;

	while let Ok(Some(msg)) = new_watcher.recv.message().await {
		match msg.message.unwrap() {
			ingest_watch_response::Message::Media(media) => {
				assert!(!media.data.is_empty());
				writer.write_all(&media.data).await.unwrap();
			}
			ingest_watch_response::Message::Ready(_) => {
				got_ready = true;
			}
			ingest_watch_response::Message::Shutdown(_) => {
				break;
			}
		}
	}

	assert!(got_ready);

	let output = ffprobe.wait_with_output().await.unwrap();
	assert!(output.status.success());

	let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

	{
		let video_stream = &json["streams"][0];
		assert_eq!(video_stream["codec_type"], "video");
		assert_eq!(video_stream["codec_name"], "h264");
		assert_eq!(video_stream["width"], 480);
		assert_eq!(video_stream["height"], 852);
		assert_eq!(video_stream["r_frame_rate"], "30/1");
		assert_eq!(video_stream["avg_frame_rate"], "30/1");
		assert_eq!(video_stream["time_base"], "1/30000");
		assert_eq!(video_stream["codec_tag"], "0x31637661");
		assert_eq!(video_stream["codec_tag_string"], "avc1");
		assert_eq!(video_stream["profile"], "High");
		assert_eq!(video_stream["level"], 31);
		assert_eq!(video_stream["refs"], 1);
		assert_eq!(video_stream["is_avc"], "true");

		let audio_stream = &json["streams"][1];
		assert_eq!(audio_stream["codec_type"], "audio");
		assert_eq!(audio_stream["codec_name"], "aac");
		assert_eq!(audio_stream["sample_rate"], "44100");
		assert_eq!(audio_stream["channels"], 1);
		assert_eq!(audio_stream["channel_layout"], "mono");
		assert_eq!(audio_stream["r_frame_rate"], "0/0");
		assert_eq!(audio_stream["avg_frame_rate"], "0/0");
		assert_eq!(audio_stream["time_base"], "1/44100");
		assert_eq!(audio_stream["codec_tag"], "0x6134706d");
		assert_eq!(audio_stream["codec_tag_string"], "mp4a");
		assert_eq!(audio_stream["profile"], "LC");
	}

	tokio::time::sleep(Duration::from_millis(200)).await;

	assert!(ffmpeg.try_wait().is_ok());

	state.finish().await;
}

#[tokio::test]
async fn test_ingest_stream_transcoder_probe_reconnect_unexpected() {
	let mut state = TestState::setup().await;
	let mut ffmpeg = stream_with_ffmpeg(
		state.rtmp_port,
		"avc_aac_keyframes.mp4",
		&generate_key(state.org_id, state.room_id),
	);

	let mut ffprobe = spawn_ffprobe();
	let writer = ffprobe.stdin.as_mut().unwrap();

	let msg = state.transcoder_request().await;

	let mut watcher = Watcher::new(msg.request_id.into_ulid(), msg.grpc_endpoint).await;

	match watcher.recv().await.message {
		Some(ingest_watch_response::Message::Media(media)) => {
			assert_eq!(media.r#type(), ingest_watch_response::media::Type::Init);
			assert!(!media.data.is_empty());
			writer.write_all(&media.data).await.unwrap();
		}
		_ => panic!("unexpected event"),
	}

	match watcher.recv().await.message {
		Some(ingest_watch_response::Message::Ready(_)) => {}
		_ => panic!("unexpected event"),
	}

	let mut i = 0;
	while let Ok(Some(msg)) = watcher.recv.message().await {
		match msg.message.unwrap() {
			ingest_watch_response::Message::Media(media) => {
				assert!(!media.data.is_empty());
				writer.write_all(&media.data).await.unwrap();
			}
			_ => panic!("unexpected event"),
		}

		i += 1;

		if i == 10 {
			break;
		}
	}

	drop(watcher);

	let output = ffprobe.wait_with_output().await.unwrap();
	assert!(output.status.success());

	let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

	{
		let video_stream = &json["streams"][0];
		assert_eq!(video_stream["codec_type"], "video");
		assert_eq!(video_stream["codec_name"], "h264");
		assert_eq!(video_stream["width"], 480);
		assert_eq!(video_stream["height"], 852);
		assert_eq!(video_stream["r_frame_rate"], "30/1");
		assert_eq!(video_stream["avg_frame_rate"], "30/1");
		assert_eq!(video_stream["time_base"], "1/30000");
		assert_eq!(video_stream["codec_tag"], "0x31637661");
		assert_eq!(video_stream["codec_tag_string"], "avc1");
		assert_eq!(video_stream["profile"], "High");
		assert_eq!(video_stream["level"], 31);
		assert_eq!(video_stream["refs"], 1);
		assert_eq!(video_stream["is_avc"], "true");

		let audio_stream = &json["streams"][1];
		assert_eq!(audio_stream["codec_type"], "audio");
		assert_eq!(audio_stream["codec_name"], "aac");
		assert_eq!(audio_stream["sample_rate"], "44100");
		assert_eq!(audio_stream["channels"], 1);
		assert_eq!(audio_stream["channel_layout"], "mono");
		assert_eq!(audio_stream["r_frame_rate"], "0/0");
		assert_eq!(audio_stream["avg_frame_rate"], "0/0");
		assert_eq!(audio_stream["time_base"], "1/44100");
		assert_eq!(audio_stream["codec_tag"], "0x6134706d");
		assert_eq!(audio_stream["codec_tag_string"], "mp4a");
		assert_eq!(audio_stream["profile"], "LC");
	}

	let msg = state.transcoder_request().await;

	let mut new_watcher = Watcher::new(msg.request_id.into_ulid(), msg.grpc_endpoint).await;

	let mut ffprobe = spawn_ffprobe();
	let writer = ffprobe.stdin.as_mut().unwrap();

	match new_watcher.recv().await.message {
		Some(ingest_watch_response::Message::Media(media)) => {
			assert_eq!(media.r#type(), ingest_watch_response::media::Type::Init);
			assert!(!media.data.is_empty());
			writer.write_all(&media.data).await.unwrap();
		}
		_ => panic!("unexpected event"),
	}

	match new_watcher.recv().await.message {
		Some(ingest_watch_response::Message::Ready(_)) => {}
		_ => panic!("unexpected event"),
	}

	while let Ok(Some(msg)) = new_watcher.recv.message().await {
		match msg.message.unwrap() {
			ingest_watch_response::Message::Media(media) => {
				assert!(!media.data.is_empty());
				writer.write_all(&media.data).await.unwrap();
			}
			ingest_watch_response::Message::Ready(_) => {
				panic!("unexpected ready");
			}
			ingest_watch_response::Message::Shutdown(_) => {
				break;
			}
		}
	}

	let output = ffprobe.wait_with_output().await.unwrap();
	assert!(output.status.success());

	let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

	{
		let video_stream = &json["streams"][0];
		assert_eq!(video_stream["codec_type"], "video");
		assert_eq!(video_stream["codec_name"], "h264");
		assert_eq!(video_stream["width"], 480);
		assert_eq!(video_stream["height"], 852);
		assert_eq!(video_stream["r_frame_rate"], "30/1");
		assert_eq!(video_stream["avg_frame_rate"], "30/1");
		assert_eq!(video_stream["time_base"], "1/30000");
		assert_eq!(video_stream["codec_tag"], "0x31637661");
		assert_eq!(video_stream["codec_tag_string"], "avc1");
		assert_eq!(video_stream["profile"], "High");
		assert_eq!(video_stream["level"], 31);
		assert_eq!(video_stream["refs"], 1);
		assert_eq!(video_stream["is_avc"], "true");

		let audio_stream = &json["streams"][1];
		assert_eq!(audio_stream["codec_type"], "audio");
		assert_eq!(audio_stream["codec_name"], "aac");
		assert_eq!(audio_stream["sample_rate"], "44100");
		assert_eq!(audio_stream["channels"], 1);
		assert_eq!(audio_stream["channel_layout"], "mono");
		assert_eq!(audio_stream["r_frame_rate"], "0/0");
		assert_eq!(audio_stream["avg_frame_rate"], "0/0");
		assert_eq!(audio_stream["time_base"], "1/44100");
		assert_eq!(audio_stream["codec_tag"], "0x6134706d");
		assert_eq!(audio_stream["codec_tag_string"], "mp4a");
		assert_eq!(audio_stream["profile"], "LC");
	}

	tokio::time::sleep(Duration::from_millis(200)).await;

	assert!(ffmpeg.try_wait().is_ok());

	state.finish().await;
}
