use std::io::{Cursor, Write};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use binary_helper::global::*;
use bytes::Bytes;
use futures_util::Stream;
use pb::ext::UlidExt;
use pb::scuffle::video::internal::events::TranscoderRequestTask;
use pb::scuffle::video::internal::ingest_server::{Ingest, IngestServer};
use pb::scuffle::video::internal::{
	ingest_watch_request, ingest_watch_response, IngestWatchRequest, IngestWatchResponse, LiveRenditionManifest,
};
use pb::scuffle::video::v1::events_fetch_request::Target;
use pb::scuffle::video::v1::types::{event, AudioConfig, Event, Rendition, VideoConfig};
use prost::Message;
use scuffle_utils::prelude::FutureTimeout;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tonic::Response;
use transmuxer::{TransmuxResult, Transmuxer};
use ulid::Ulid;
use video_common::database::{Room, RoomStatus};
use video_common::ext::AsyncReadExt as _;

use super::global::GlobalState;
use crate::config::TranscoderConfig;
use crate::global::TranscoderState;
use crate::transcoder;

type IngestRequest = (
	mpsc::Sender<Result<IngestWatchResponse>>,
	tonic::Streaming<IngestWatchRequest>,
);

struct ImplIngestServer {
	tx: mpsc::Sender<IngestRequest>,
}

type Result<T> = std::result::Result<T, tonic::Status>;

#[async_trait]
impl Ingest for ImplIngestServer {
	type WatchStream = Pin<Box<dyn Stream<Item = Result<IngestWatchResponse>> + 'static + Send>>;

	async fn watch(
		&self,
		request: tonic::Request<tonic::Streaming<IngestWatchRequest>>,
	) -> Result<Response<Self::WatchStream>> {
		let (tx, rx) = mpsc::channel(16);
		self.tx.send((tx, request.into_inner())).await.unwrap();
		Ok(Response::new(Box::pin(ReceiverStream::new(rx))))
	}
}

fn setup_ingest_server(global: Arc<GlobalState>, bind: impl Into<SocketAddr>) -> mpsc::Receiver<IngestRequest> {
	let (tx, rx) = mpsc::channel(256);
	let server = ImplIngestServer { tx };
	let bind = bind.into();

	tokio::spawn(async move {
		tonic::transport::Server::builder()
			.add_service(IngestServer::new(server))
			.serve_with_shutdown(bind, async move {
				global.ctx().done().await;
			})
			.await
			.unwrap();
	});

	rx
}

#[tokio::test]
async fn test_transcode() {
	let port = portpicker::pick_unused_port().unwrap();

	let (global, handler) = crate::tests::global::mock_global_state(TranscoderConfig {
		events_stream_name: Ulid::new().to_string(),
		transcoder_request_subject: Ulid::new().to_string(),
		metadata_kv_store: Ulid::new().to_string(),
		media_ob_store: Ulid::new().to_string(),
		..Default::default()
	})
	.await;

	let org_id = Ulid::new();

	let mut event_stream = global
		.nats()
		.subscribe(video_common::keys::event_subject(
			&global.config().events_stream_name,
			org_id,
			Target::Room,
		))
		.await
		.unwrap();

	let addr = SocketAddr::from(([127, 0, 0, 1], port));

	let mut rx = setup_ingest_server(global.clone(), addr);

	let transcoder_run_handle = tokio::spawn(transcoder::run(global.clone()));

	let req_id = Ulid::new();

	let room_id = Ulid::new();
	let connection_id = Ulid::new();

	scuffle_utils::database::query(
		r#"
    INSERT INTO organizations (
        id,
        name
    ) VALUES (
        $1,
        $2
    )"#,
	)
	.bind(org_id)
	.bind(room_id.to_string())
	.build()
	.execute(global.db())
	.await
	.unwrap();

	scuffle_utils::database::query(
		r#"
    INSERT INTO rooms (
        id,
        organization_id,
        active_ingest_connection_id,
        stream_key,
        video_input,
        audio_input
    ) VALUES (
        $1,
        $2,
        $3,
        $4,
        $5,
        $6
    )"#,
	)
	.bind(room_id)
	.bind(org_id)
	.bind(connection_id)
	.bind(room_id.to_string())
	.bind(
		VideoConfig {
			bitrate: 7358 * 1024,
			codec: "avc1.64002a".to_string(),
			fps: 60,
			height: 2160,
			width: 3840,
			rendition: Rendition::VideoSource.into(),
		}
		.encode_to_vec(),
	)
	.bind(
		AudioConfig {
			bitrate: 130 * 1024,
			codec: "mp4a.40.2".to_string(),
			channels: 2,
			sample_rate: 48000,
			rendition: Rendition::AudioSource.into(),
		}
		.encode_to_vec(),
	)
	.build()
	.execute(global.db())
	.await
	.unwrap();

	let config = global.config::<TranscoderConfig>();

	global
		.nats()
		.publish(
			config.transcoder_request_subject.clone(),
			TranscoderRequestTask {
				room_id: Some(room_id.into()),
				organization_id: Some(org_id.into()),
				request_id: Some(req_id.into()),
				connection_id: Some(connection_id.into()),
				grpc_endpoint: format!("localhost:{}", port),
			}
			.encode_to_vec()
			.into(),
		)
		.await
		.unwrap();

	let (sender, receiver) = rx.recv().timeout(Duration::from_secs(2)).await.unwrap().unwrap();

	// This is now a stream we can write frames to.
	// We now need to demux the video into fragmnts to send to the transcoder.
	let flv_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
		.join("../assets")
		.join("avc_aac.flv");
	let data = std::fs::read(&flv_path).unwrap();

	let mut cursor = Cursor::new(Bytes::from(data));
	let mut transmuxer = Transmuxer::new();

	let flv = flv::Flv::demux(&mut cursor).unwrap();

	let mut video = None;
	let mut audio = None;

	for tag in flv.tags {
		transmuxer.add_tag(tag);
		tracing::debug!("added tag");
		// We dont want to send too many frames at once, so we sleep a bit.
		tokio::time::sleep(Duration::from_millis(10)).await;
		if let Some(data) = transmuxer.mux().unwrap() {
			match data {
				TransmuxResult::InitSegment {
					data,
					video_settings,
					audio_settings,
				} => {
					video = Some(video_settings);
					audio = Some(audio_settings);
					sender
						.send(Ok(IngestWatchResponse {
							message: Some(ingest_watch_response::Message::Media(ingest_watch_response::Media {
								data,
								keyframe: false,
								timescale: 0,
								timestamp: 0,
								r#type: ingest_watch_response::media::Type::Init.into(),
							})),
						}))
						.await
						.unwrap();
					sender
						.send(Ok(IngestWatchResponse {
							message: Some(ingest_watch_response::Message::Ready(
								ingest_watch_response::Ready::Ready.into(),
							)),
						}))
						.await
						.unwrap();
				}
				TransmuxResult::MediaSegment(ms) => {
					sender
						.send(Ok(IngestWatchResponse {
							message: Some(ingest_watch_response::Message::Media(ingest_watch_response::Media {
								data: ms.data,
								keyframe: ms.keyframe,
								timescale: match ms.ty {
									transmuxer::MediaType::Audio => audio.as_ref().unwrap().timescale,
									transmuxer::MediaType::Video => video.as_ref().unwrap().timescale,
								},
								timestamp: ms.timestamp,
								r#type: match ms.ty {
									transmuxer::MediaType::Audio => ingest_watch_response::media::Type::Audio.into(),
									transmuxer::MediaType::Video => ingest_watch_response::media::Type::Video.into(),
								},
							})),
						}))
						.await
						.unwrap();
				}
			}
		}
	}

	{
		let event = Event::decode(event_stream.next().await.unwrap().payload).unwrap();
		assert!(event.timestamp - chrono::Utc::now().timestamp_millis() < 1000);
		match event.event {
			Some(event::Event::Room(room)) => {
				assert_eq!(room.room_id.into_ulid(), room_id);

				match room.event {
					Some(event::room::Event::Ready(ready)) => {
						assert_eq!(ready.connection_id.into_ulid(), connection_id);
					}
					_ => panic!("unexpected event"),
				}
			}
			_ => panic!("unexpected event"),
		};
	}

	tokio::time::sleep(Duration::from_millis(100)).await;

	let video_manifest = LiveRenditionManifest::decode(
		global
			.metadata_store()
			.get(video_common::keys::rendition_manifest(
				org_id,
				room_id,
				connection_id,
				Rendition::VideoSource.into(),
			))
			.await
			.unwrap()
			.unwrap(),
	)
	.unwrap();
	let audio_manifest = LiveRenditionManifest::decode(
		global
			.metadata_store()
			.get(video_common::keys::rendition_manifest(
				org_id,
				room_id,
				connection_id,
				Rendition::AudioSource.into(),
			))
			.await
			.unwrap()
			.unwrap(),
	)
	.unwrap();

	assert_eq!(video_manifest.segments.len(), 1);
	assert_eq!(video_manifest.segments[0].parts.len(), 3);
	assert!(video_manifest.segments[0].parts.iter().skip(1).all(|p| !p.independent));
	assert!(video_manifest.segments[0].parts[0].independent);
	assert!(!video_manifest.completed);
	assert_eq!(video_manifest.info.as_ref().unwrap().next_segment_idx, 1);
	assert_eq!(video_manifest.info.as_ref().unwrap().next_part_idx, 3);

	assert_eq!(audio_manifest.segments.len(), 1);
	assert_eq!(audio_manifest.segments[0].parts.len(), 3);
	assert!(audio_manifest.segments[0].parts.iter().all(|p| p.independent));
	assert!(!audio_manifest.completed);
	assert_eq!(audio_manifest.info.as_ref().unwrap().next_segment_idx, 1);
	assert_eq!(audio_manifest.info.as_ref().unwrap().next_part_idx, 3);

	tracing::debug!("finished sending frames");

	sender
		.send(Ok(IngestWatchResponse {
			message: Some(ingest_watch_response::Message::Shutdown(
				ingest_watch_response::Shutdown::Stream.into(),
			)),
		}))
		.await
		.unwrap();

	drop(sender);
	drop(receiver);

	tokio::time::sleep(Duration::from_millis(250)).await;

	let video_manifest = LiveRenditionManifest::decode(
		global
			.metadata_store()
			.get(video_common::keys::rendition_manifest(
				org_id,
				room_id,
				connection_id,
				Rendition::VideoSource.into(),
			))
			.await
			.unwrap()
			.unwrap(),
	)
	.unwrap();
	let audio_manifest = LiveRenditionManifest::decode(
		global
			.metadata_store()
			.get(video_common::keys::rendition_manifest(
				org_id,
				room_id,
				connection_id,
				Rendition::AudioSource.into(),
			))
			.await
			.unwrap()
			.unwrap(),
	)
	.unwrap();

	assert_eq!(video_manifest.segments.len(), 1);
	assert_eq!(video_manifest.segments[0].parts.len(), 4);
	assert!(video_manifest.segments[0].parts.iter().skip(1).all(|p| !p.independent));
	assert!(video_manifest.segments[0].parts[0].independent);
	assert!(video_manifest.completed);
	assert_eq!(video_manifest.info.as_ref().unwrap().next_segment_idx, 1);
	assert_eq!(video_manifest.info.as_ref().unwrap().next_part_idx, 4);
	assert_eq!(video_manifest.total_duration, 59000); // verified with ffprobe

	assert_eq!(video_manifest.segments.len(), 1);
	assert_eq!(audio_manifest.segments[0].parts.len(), 4);
	assert!(audio_manifest.segments[0].parts.iter().all(|p| p.independent));
	assert!(audio_manifest.completed);
	assert_eq!(audio_manifest.info.as_ref().unwrap().next_segment_idx, 1);
	assert_eq!(audio_manifest.info.as_ref().unwrap().next_part_idx, 4);
	assert_eq!(audio_manifest.total_duration, 48128); // verified with ffprobe

	let mut video_parts = vec![
		global
			.media_store()
			.get(video_common::keys::init(
				org_id,
				room_id,
				connection_id,
				Rendition::VideoSource.into(),
			))
			.await
			.unwrap()
			.read_all()
			.await
			.unwrap(),
	];
	let mut audio_parts = vec![
		global
			.media_store()
			.get(video_common::keys::init(
				org_id,
				room_id,
				connection_id,
				Rendition::AudioSource.into(),
			))
			.await
			.unwrap()
			.read_all()
			.await
			.unwrap(),
	];

	for i in 1..=3 {
		video_parts.push(
			global
				.media_store()
				.get(video_common::keys::part(
					org_id,
					room_id,
					connection_id,
					Rendition::VideoSource.into(),
					i,
				))
				.await
				.unwrap()
				.read_all()
				.await
				.unwrap(),
		);
		audio_parts.push(
			global
				.media_store()
				.get(video_common::keys::part(
					org_id,
					room_id,
					connection_id,
					Rendition::AudioSource.into(),
					i,
				))
				.await
				.unwrap()
				.read_all()
				.await
				.unwrap(),
		);
	}

	let mut tmp_file = tempfile::NamedTempFile::new().unwrap();
	tmp_file.write_all(&video_parts.concat()).unwrap();

	let command = Command::new("ffprobe")
		.arg("-v")
		.arg("error")
		.arg("-fpsprobesize")
		.arg("20000000")
		.arg("-show_format")
		.arg("-show_streams")
		.arg("-print_format")
		.arg("json")
		.arg(tmp_file.path().to_str().unwrap())
		.stdin(Stdio::null())
		.stdout(Stdio::piped())
		.stderr(Stdio::inherit())
		.spawn()
		.unwrap();

	let output = command.wait_with_output().await.unwrap();
	let json = serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap();

	println!("{:#?}", json);

	assert_eq!(json["format"]["format_name"], "mov,mp4,m4a,3gp,3g2,mj2");
	assert_eq!(json["format"]["tags"]["major_brand"], "iso5");
	assert_eq!(json["format"]["tags"]["minor_version"], "512");
	assert_eq!(json["format"]["tags"]["compatible_brands"], "iso5iso6mp41");

	assert_eq!(json["streams"][0]["codec_name"], "h264");
	assert_eq!(json["streams"][0]["codec_type"], "video");
	assert_eq!(json["streams"][0]["width"], 3840);
	assert_eq!(json["streams"][0]["height"], 2160);
	assert_eq!(json["streams"][0]["r_frame_rate"], "60/1");
	assert_eq!(json["streams"][0]["avg_frame_rate"], "60/1");
	assert_eq!(json["streams"][0]["duration_ts"], 59000);
	assert_eq!(json["streams"][0]["time_base"], "1/60000");
	assert_eq!(json["streams"][0]["duration"], "0.983333");

	let mut tmp_file = tempfile::NamedTempFile::new().unwrap();
	tmp_file.write_all(&audio_parts.concat()).unwrap();

	let command = Command::new("ffprobe")
		.arg("-v")
		.arg("error")
		.arg("-fpsprobesize")
		.arg("20000000")
		.arg("-probesize")
		.arg("20000000")
		.arg("-analyzeduration")
		.arg("20000000")
		.arg("-show_format")
		.arg("-show_streams")
		.arg("-print_format")
		.arg("json")
		.arg(tmp_file.path().to_str().unwrap())
		.stdin(Stdio::null())
		.stdout(Stdio::piped())
		.stderr(Stdio::inherit())
		.spawn()
		.unwrap();

	let output = command.wait_with_output().await.unwrap();
	let json = serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap();

	println!("{:#?}", json);

	assert_eq!(json["format"]["format_name"], "mov,mp4,m4a,3gp,3g2,mj2");
	assert_eq!(json["format"]["tags"]["major_brand"], "iso5");
	assert_eq!(json["format"]["tags"]["minor_version"], "512");
	assert_eq!(json["format"]["tags"]["compatible_brands"], "iso5iso6mp41");

	assert_eq!(json["streams"][0]["codec_name"], "aac");
	assert_eq!(json["streams"][0]["codec_type"], "audio");
	assert_eq!(json["streams"][0]["sample_rate"], "48000");
	assert_eq!(json["streams"][0]["channels"], 2);
	assert_eq!(json["streams"][0]["duration_ts"], 48128);
	assert_eq!(json["streams"][0]["time_base"], "1/48000");

	let room: Room = scuffle_utils::database::query(
		"SELECT * FROM rooms WHERE organization_id = $1 AND id = $2 AND active_ingest_connection_id = $3",
	)
	.bind(org_id)
	.bind(room_id)
	.bind(connection_id)
	.build_query_as()
	.fetch_one(global.db())
	.await
	.unwrap();

	let active_transcoding_config = room.active_transcoding_config.unwrap();
	assert!(room.active_recording_config.is_none());
	let video_output = room.video_output.unwrap();
	let audio_output = room.audio_output.unwrap();

	assert!(
		active_transcoding_config
			.renditions
			.contains(&(Rendition::VideoSource as i32))
	);
	assert!(
		active_transcoding_config
			.renditions
			.contains(&(Rendition::AudioSource as i32))
	);
	assert_eq!(active_transcoding_config.id.into_ulid(), Ulid::nil());
	assert_eq!(active_transcoding_config.created_at, 0);

	assert_eq!(video_output.len(), 1);
	assert_eq!(audio_output.len(), 1);

	assert_eq!(video_output[0].rendition, Rendition::VideoSource as i32);
	assert_eq!(video_output[0].codec, "avc1.64002a");
	assert_eq!(video_output[0].bitrate, 7358 * 1024);
	assert_eq!(video_output[0].fps, 60);
	assert_eq!(video_output[0].height, 2160);
	assert_eq!(video_output[0].width, 3840);

	assert_eq!(audio_output[0].rendition, Rendition::AudioSource as i32);
	assert_eq!(audio_output[0].codec, "mp4a.40.2");
	assert_eq!(audio_output[0].bitrate, 130 * 1024);
	assert_eq!(audio_output[0].channels, 2);
	assert_eq!(audio_output[0].sample_rate, 48000);

	assert_eq!(room.status, RoomStatus::Ready);

	drop(global);
	handler.cancel().timeout(Duration::from_secs(2)).await.unwrap();
	transcoder_run_handle
		.timeout(Duration::from_secs(2))
		.await
		.unwrap()
		.unwrap()
		.unwrap();

	tracing::info!("done");
}

#[tokio::test]
async fn test_transcode_reconnect() {
	let port = portpicker::pick_unused_port().unwrap();

	let (global, handler) = crate::tests::global::mock_global_state(TranscoderConfig {
		events_stream_name: Ulid::new().to_string(),
		transcoder_request_subject: Ulid::new().to_string(),
		metadata_kv_store: Ulid::new().to_string(),
		media_ob_store: Ulid::new().to_string(),
		..Default::default()
	})
	.await;

	let config = global.config::<TranscoderConfig>();

	global
		.jetstream()
		.create_stream(async_nats::jetstream::stream::Config {
			name: config.transcoder_request_subject.clone(),
			..Default::default()
		})
		.await
		.unwrap();

	let org_id = Ulid::new();

	let mut event_stream = global
		.nats()
		.subscribe(video_common::keys::event_subject(
			&global.config().events_stream_name,
			org_id,
			Target::Room,
		))
		.await
		.unwrap();

	let addr = SocketAddr::from(([127, 0, 0, 1], port));

	let mut rx = setup_ingest_server(global.clone(), addr);

	let transcoder_run_handle = tokio::spawn(transcoder::run(global.clone()));

	let req_id = Ulid::new();

	let room_id = Ulid::new();
	let connection_id = Ulid::new();

	scuffle_utils::database::query(
		r#"
    INSERT INTO organizations (
        id,
        name
    ) VALUES (
        $1,
        $2
    )"#,
	)
	.bind(org_id)
	.bind(room_id.to_string())
	.build()
	.execute(global.db())
	.await
	.unwrap();

	scuffle_utils::database::query(
		r#"
    INSERT INTO rooms (
        organization_id,
        id,
        active_ingest_connection_id,
        stream_key,
        video_input,
        audio_input
    ) VALUES (
        $1,
        $2,
        $3,
        $4,
        $5,
        $6
    )"#,
	)
	.bind(org_id)
	.bind(room_id)
	.bind(connection_id)
	.bind(room_id.to_string())
	.bind(
		VideoConfig {
			bitrate: 7358 * 1024,
			codec: "avc1.64002a".to_string(),
			fps: 60,
			height: 3840,
			width: 2160,
			rendition: Rendition::VideoSource.into(),
		}
		.encode_to_vec(),
	)
	.bind(
		AudioConfig {
			bitrate: 130 * 1024,
			codec: "mp4a.40.2".to_string(),
			channels: 2,
			sample_rate: 48000,
			rendition: Rendition::AudioSource.into(),
		}
		.encode_to_vec(),
	)
	.build()
	.execute(global.db())
	.await
	.unwrap();

	// This is now a stream we can write frames to.
	// We now need to demux the video into fragmnts to send to the transcoder.
	let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../assets");
	let data = std::fs::read(dir.join("avc_aac.flv").to_str().unwrap()).unwrap();

	let mut cursor = Cursor::new(Bytes::from(data));
	let mut transmuxer = Transmuxer::new();

	let flv = flv::Flv::demux(&mut cursor).unwrap();

	flv.tags.into_iter().for_each(|t| {
		transmuxer.add_tag(t);
	});

	let mut packets = vec![];
	while let Some(packet) = transmuxer.mux().unwrap() {
		packets.push(packet);
	}

	let config = global.config::<TranscoderConfig>();

	{
		global
			.nats()
			.publish(
				config.transcoder_request_subject.clone(),
				TranscoderRequestTask {
					room_id: Some(room_id.into()),
					organization_id: Some(org_id.into()),
					request_id: Some(req_id.into()),
					connection_id: Some(connection_id.into()),
					grpc_endpoint: format!("localhost:{}", port),
				}
				.encode_to_vec()
				.into(),
			)
			.await
			.unwrap();

		let (sender, mut receiver) = rx.recv().timeout(Duration::from_secs(2)).await.unwrap().unwrap();

		assert_eq!(
			receiver.message().await.unwrap().unwrap().message.unwrap(),
			ingest_watch_request::Message::Open(ingest_watch_request::Open {
				request_id: Some(req_id.into()),
			})
		);

		let mut audio = None;
		let mut video = None;

		for packet in &packets {
			match packet {
				TransmuxResult::InitSegment {
					data,
					audio_settings,
					video_settings,
				} => {
					audio = Some(audio_settings);
					video = Some(video_settings);
					sender
						.send(Ok(IngestWatchResponse {
							message: Some(ingest_watch_response::Message::Media(ingest_watch_response::Media {
								data: data.clone(),
								keyframe: false,
								timescale: 0,
								timestamp: 0,
								r#type: ingest_watch_response::media::Type::Init.into(),
							})),
						}))
						.await
						.unwrap();

					sender
						.send(Ok(IngestWatchResponse {
							message: Some(ingest_watch_response::Message::Ready(
								ingest_watch_response::Ready::Ready.into(),
							)),
						}))
						.await
						.unwrap();
				}
				TransmuxResult::MediaSegment(ms) => {
					tokio::time::sleep(Duration::from_millis(10)).await;

					sender
						.send(Ok(IngestWatchResponse {
							message: Some(ingest_watch_response::Message::Media(ingest_watch_response::Media {
								data: ms.data.clone(),
								keyframe: ms.keyframe,
								timescale: match ms.ty {
									transmuxer::MediaType::Audio => audio.as_ref().unwrap().timescale,
									transmuxer::MediaType::Video => video.as_ref().unwrap().timescale,
								},
								timestamp: ms.timestamp,
								r#type: match ms.ty {
									transmuxer::MediaType::Audio => ingest_watch_response::media::Type::Audio.into(),
									transmuxer::MediaType::Video => ingest_watch_response::media::Type::Video.into(),
								},
							})),
						}))
						.await
						.unwrap();
				}
			}
		}

		{
			let event = Event::decode(event_stream.next().await.unwrap().payload).unwrap();
			assert!(event.timestamp - chrono::Utc::now().timestamp_millis() < 1000);
			match event.event {
				Some(event::Event::Room(room)) => {
					assert_eq!(room.room_id.into_ulid(), room_id);

					match room.event {
						Some(event::room::Event::Ready(ready)) => {
							assert_eq!(ready.connection_id.into_ulid(), connection_id);
						}
						_ => panic!("unexpected event"),
					}
				}
				_ => panic!("unexpected event"),
			};
		}

		sender
			.send(Ok(IngestWatchResponse {
				message: Some(ingest_watch_response::Message::Shutdown(
					ingest_watch_response::Shutdown::Transcoder.into(),
				)),
			}))
			.await
			.unwrap();
		assert_eq!(
			receiver.message().await.unwrap().unwrap().message.unwrap(),
			ingest_watch_request::Message::Shutdown(ingest_watch_request::Shutdown::Complete.into())
		);

		let video_manifest = LiveRenditionManifest::decode(
			global
				.metadata_store()
				.get(video_common::keys::rendition_manifest(
					org_id,
					room_id,
					connection_id,
					Rendition::VideoSource.into(),
				))
				.await
				.unwrap()
				.unwrap(),
		)
		.unwrap();
		let audio_manifest = LiveRenditionManifest::decode(
			global
				.metadata_store()
				.get(video_common::keys::rendition_manifest(
					org_id,
					room_id,
					connection_id,
					Rendition::AudioSource.into(),
				))
				.await
				.unwrap()
				.unwrap(),
		)
		.unwrap();

		assert_eq!(video_manifest.segments.len(), 1);
		assert_eq!(video_manifest.segments[0].parts.len(), 4);
		assert!(video_manifest.segments[0].parts.iter().skip(1).all(|p| !p.independent));
		assert!(video_manifest.segments[0].parts[0].independent);
		assert!(!video_manifest.completed);
		assert_eq!(video_manifest.info.as_ref().unwrap().next_segment_idx, 1);
		assert_eq!(video_manifest.info.as_ref().unwrap().next_part_idx, 4);
		assert_eq!(video_manifest.info.as_ref().unwrap().next_segment_part_idx, 0);
		assert_eq!(video_manifest.total_duration, 59000); // verified with ffprobe

		assert_eq!(video_manifest.segments.len(), 1);
		assert_eq!(audio_manifest.segments[0].parts.len(), 4);
		assert!(audio_manifest.segments[0].parts.iter().all(|p| p.independent));
		assert!(!audio_manifest.completed);
		assert_eq!(audio_manifest.info.as_ref().unwrap().next_segment_idx, 1);
		assert_eq!(audio_manifest.info.as_ref().unwrap().next_part_idx, 4);
		assert_eq!(audio_manifest.total_duration, 48128); // verified with ffprobe
	}

	{
		let new_req_id = Ulid::new();

		global
			.nats()
			.publish(
				config.transcoder_request_subject.clone(),
				TranscoderRequestTask {
					room_id: Some(room_id.into()),
					organization_id: Some(org_id.into()),
					request_id: Some(new_req_id.into()),
					connection_id: Some(connection_id.into()),
					grpc_endpoint: format!("localhost:{}", port),
				}
				.encode_to_vec()
				.into(),
			)
			.await
			.unwrap();

		let (sender, mut receiver) = rx.recv().timeout(Duration::from_secs(2)).await.unwrap().unwrap();

		assert_eq!(
			receiver.message().await.unwrap().unwrap().message.unwrap(),
			ingest_watch_request::Message::Open(ingest_watch_request::Open {
				request_id: Some(new_req_id.into()),
			})
		);

		let mut audio = None;
		let mut video = None;

		for packet in &packets {
			match packet {
				TransmuxResult::InitSegment {
					data,
					audio_settings,
					video_settings,
				} => {
					audio = Some(audio_settings);
					video = Some(video_settings);
					sender
						.send(Ok(IngestWatchResponse {
							message: Some(ingest_watch_response::Message::Media(ingest_watch_response::Media {
								data: data.clone(),
								keyframe: false,
								timescale: 0,
								timestamp: 0,
								r#type: ingest_watch_response::media::Type::Init.into(),
							})),
						}))
						.await
						.unwrap();

					sender
						.send(Ok(IngestWatchResponse {
							message: Some(ingest_watch_response::Message::Ready(
								ingest_watch_response::Ready::Ready.into(),
							)),
						}))
						.await
						.unwrap();
				}
				TransmuxResult::MediaSegment(ms) => {
					tokio::time::sleep(Duration::from_millis(10)).await;

					sender
						.send(Ok(IngestWatchResponse {
							message: Some(ingest_watch_response::Message::Media(ingest_watch_response::Media {
								data: ms.data.clone(),
								keyframe: ms.keyframe,
								timescale: match ms.ty {
									transmuxer::MediaType::Audio => audio.as_ref().unwrap().timescale,
									transmuxer::MediaType::Video => video.as_ref().unwrap().timescale,
								},
								timestamp: ms.timestamp,
								r#type: match ms.ty {
									transmuxer::MediaType::Audio => ingest_watch_response::media::Type::Audio.into(),
									transmuxer::MediaType::Video => ingest_watch_response::media::Type::Video.into(),
								},
							})),
						}))
						.await
						.unwrap();
				}
			}
		}

		{
			let event = Event::decode(event_stream.next().await.unwrap().payload).unwrap();
			assert!(event.timestamp - chrono::Utc::now().timestamp_millis() < 1000);
			match event.event {
				Some(event::Event::Room(room)) => {
					assert_eq!(room.room_id.into_ulid(), room_id);

					match room.event {
						Some(event::room::Event::Ready(ready)) => {
							assert_eq!(ready.connection_id.into_ulid(), connection_id);
						}
						_ => panic!("unexpected event"),
					}
				}
				_ => panic!("unexpected event"),
			};
		}

		sender
			.send(Ok(IngestWatchResponse {
				message: Some(ingest_watch_response::Message::Shutdown(
					ingest_watch_response::Shutdown::Transcoder.into(),
				)),
			}))
			.await
			.unwrap();
		assert_eq!(
			receiver.message().await.unwrap().unwrap().message.unwrap(),
			ingest_watch_request::Message::Shutdown(ingest_watch_request::Shutdown::Complete.into())
		);

		let video_manifest = LiveRenditionManifest::decode(
			global
				.metadata_store()
				.get(video_common::keys::rendition_manifest(
					org_id,
					room_id,
					connection_id,
					Rendition::VideoSource.into(),
				))
				.await
				.unwrap()
				.unwrap(),
		)
		.unwrap();
		let audio_manifest = LiveRenditionManifest::decode(
			global
				.metadata_store()
				.get(video_common::keys::rendition_manifest(
					org_id,
					room_id,
					connection_id,
					Rendition::AudioSource.into(),
				))
				.await
				.unwrap()
				.unwrap(),
		)
		.unwrap();

		dbg!(&video_manifest);

		assert_eq!(video_manifest.segments.len(), 2);
		assert_eq!(video_manifest.segments[0].parts.len(), 4);
		assert_eq!(video_manifest.segments[1].parts.len(), 4);
		assert_eq!(video_manifest.segments[0].parts.iter().filter(|p| p.independent).count(), 1);
		assert_eq!(video_manifest.segments[1].parts.iter().filter(|p| p.independent).count(), 1);
		assert!(video_manifest.segments[0].parts[0].independent);
		assert!(video_manifest.segments[1].parts[0].independent);
		assert!(!video_manifest.completed);
		assert_eq!(video_manifest.info.as_ref().unwrap().next_segment_idx, 2);
		assert_eq!(video_manifest.info.as_ref().unwrap().next_part_idx, 8);
		assert_eq!(video_manifest.info.as_ref().unwrap().next_segment_part_idx, 0);
		assert_eq!(video_manifest.total_duration, 59000 * 2); // verified with ffprobe

		assert_eq!(video_manifest.segments.len(), 2);
		assert_eq!(audio_manifest.segments[0].parts.len(), 4);
		assert_eq!(audio_manifest.segments[1].parts.len(), 4);
		assert!(audio_manifest.segments[0].parts.iter().all(|p| p.independent));
		assert!(audio_manifest.segments[1].parts.iter().all(|p| p.independent));
		assert!(!audio_manifest.completed);
		assert_eq!(audio_manifest.info.as_ref().unwrap().next_segment_idx, 2);
		assert_eq!(audio_manifest.info.as_ref().unwrap().next_part_idx, 8);
		assert_eq!(audio_manifest.info.as_ref().unwrap().next_segment_part_idx, 0);
		assert_eq!(audio_manifest.total_duration, 48128 * 2); // verified with ffprobe
	}

	{
		let new_req_id = Ulid::new();

		global
			.nats()
			.publish(
				config.transcoder_request_subject.clone(),
				TranscoderRequestTask {
					room_id: Some(room_id.into()),
					organization_id: Some(org_id.into()),
					request_id: Some(new_req_id.into()),
					connection_id: Some(connection_id.into()),
					grpc_endpoint: format!("localhost:{}", port),
				}
				.encode_to_vec()
				.into(),
			)
			.await
			.unwrap();

		let (sender, mut receiver) = rx.recv().timeout(Duration::from_secs(2)).await.unwrap().unwrap();

		assert_eq!(
			receiver.message().await.unwrap().unwrap().message.unwrap(),
			ingest_watch_request::Message::Open(ingest_watch_request::Open {
				request_id: Some(new_req_id.into()),
			})
		);

		let mut audio = None;
		let mut video = None;

		for packet in &packets {
			match packet {
				TransmuxResult::InitSegment {
					data,
					audio_settings,
					video_settings,
				} => {
					audio = Some(audio_settings);
					video = Some(video_settings);
					sender
						.send(Ok(IngestWatchResponse {
							message: Some(ingest_watch_response::Message::Media(ingest_watch_response::Media {
								data: data.clone(),
								keyframe: false,
								timescale: 0,
								timestamp: 0,
								r#type: ingest_watch_response::media::Type::Init.into(),
							})),
						}))
						.await
						.unwrap();

					sender
						.send(Ok(IngestWatchResponse {
							message: Some(ingest_watch_response::Message::Ready(
								ingest_watch_response::Ready::Ready.into(),
							)),
						}))
						.await
						.unwrap();
				}
				TransmuxResult::MediaSegment(ms) => {
					tokio::time::sleep(Duration::from_millis(10)).await;

					sender
						.send(Ok(IngestWatchResponse {
							message: Some(ingest_watch_response::Message::Media(ingest_watch_response::Media {
								data: ms.data.clone(),
								keyframe: ms.keyframe,
								timescale: match ms.ty {
									transmuxer::MediaType::Audio => audio.as_ref().unwrap().timescale,
									transmuxer::MediaType::Video => video.as_ref().unwrap().timescale,
								},
								timestamp: ms.timestamp,
								r#type: match ms.ty {
									transmuxer::MediaType::Audio => ingest_watch_response::media::Type::Audio.into(),
									transmuxer::MediaType::Video => ingest_watch_response::media::Type::Video.into(),
								},
							})),
						}))
						.await
						.unwrap();
				}
			}
		}

		{
			let event = Event::decode(event_stream.next().await.unwrap().payload).unwrap();
			assert!(event.timestamp - chrono::Utc::now().timestamp_millis() < 1000);
			match event.event {
				Some(event::Event::Room(room)) => {
					assert_eq!(room.room_id.into_ulid(), room_id);

					match room.event {
						Some(event::room::Event::Ready(ready)) => {
							assert_eq!(ready.connection_id.into_ulid(), connection_id);
						}
						_ => panic!("unexpected event"),
					}
				}
				_ => panic!("unexpected event"),
			};
		}

		sender
			.send(Ok(IngestWatchResponse {
				message: Some(ingest_watch_response::Message::Shutdown(
					ingest_watch_response::Shutdown::Transcoder.into(),
				)),
			}))
			.await
			.unwrap();
		assert_eq!(
			receiver.message().await.unwrap().unwrap().message.unwrap(),
			ingest_watch_request::Message::Shutdown(ingest_watch_request::Shutdown::Complete.into())
		);

		let video_manifest = LiveRenditionManifest::decode(
			global
				.metadata_store()
				.get(video_common::keys::rendition_manifest(
					org_id,
					room_id,
					connection_id,
					Rendition::VideoSource.into(),
				))
				.await
				.unwrap()
				.unwrap(),
		)
		.unwrap();
		let audio_manifest = LiveRenditionManifest::decode(
			global
				.metadata_store()
				.get(video_common::keys::rendition_manifest(
					org_id,
					room_id,
					connection_id,
					Rendition::AudioSource.into(),
				))
				.await
				.unwrap()
				.unwrap(),
		)
		.unwrap();

		assert_eq!(video_manifest.segments.len(), 3);
		assert_eq!(video_manifest.segments[0].parts.len(), 4);
		assert_eq!(video_manifest.segments[1].parts.len(), 4);
		assert_eq!(video_manifest.segments[2].parts.len(), 4);
		assert_eq!(video_manifest.segments[0].parts.iter().filter(|p| p.independent).count(), 1);
		assert_eq!(video_manifest.segments[1].parts.iter().filter(|p| p.independent).count(), 1);
		assert_eq!(video_manifest.segments[2].parts.iter().filter(|p| p.independent).count(), 1);
		assert!(video_manifest.segments[0].parts[0].independent);
		assert!(video_manifest.segments[1].parts[0].independent);
		assert!(video_manifest.segments[2].parts[0].independent);
		assert!(!video_manifest.completed);
		assert_eq!(video_manifest.info.as_ref().unwrap().next_segment_idx, 3);
		assert_eq!(video_manifest.info.as_ref().unwrap().next_part_idx, 12);
		assert_eq!(video_manifest.info.as_ref().unwrap().next_segment_part_idx, 0);
		assert_eq!(video_manifest.other_info["audio_source"].next_segment_idx, 3);
		assert_eq!(video_manifest.other_info["audio_source"].next_part_idx, 12);
		assert_eq!(video_manifest.other_info["audio_source"].next_segment_part_idx, 0);
		assert_eq!(video_manifest.total_duration, 59000 * 3); // verified with ffprobe

		assert_eq!(audio_manifest.segments.len(), 3);
		assert_eq!(audio_manifest.segments[0].parts.len(), 4);
		assert_eq!(audio_manifest.segments[1].parts.len(), 4);
		assert_eq!(audio_manifest.segments[2].parts.len(), 4);
		assert!(audio_manifest.segments[0].parts.iter().all(|p| p.independent));
		assert!(audio_manifest.segments[1].parts.iter().all(|p| p.independent));
		assert!(audio_manifest.segments[2].parts.iter().all(|p| p.independent));
		assert!(!audio_manifest.completed);
		assert_eq!(audio_manifest.info.as_ref().unwrap().next_segment_idx, 3);
		assert_eq!(audio_manifest.info.as_ref().unwrap().next_part_idx, 12);
		assert_eq!(audio_manifest.info.as_ref().unwrap().next_segment_part_idx, 0);
		assert_eq!(audio_manifest.other_info["video_source"].next_segment_idx, 3);
		assert_eq!(audio_manifest.other_info["video_source"].next_part_idx, 12);
		assert_eq!(audio_manifest.other_info["video_source"].next_segment_part_idx, 0);
		assert_eq!(audio_manifest.total_duration, 48128 * 3); // verified with
		// ffprobe
	}

	{
		let new_req_id = Ulid::new();

		global
			.nats()
			.publish(
				config.transcoder_request_subject.clone(),
				TranscoderRequestTask {
					room_id: Some(room_id.into()),
					organization_id: Some(org_id.into()),
					request_id: Some(new_req_id.into()),
					connection_id: Some(connection_id.into()),
					grpc_endpoint: format!("localhost:{}", port),
				}
				.encode_to_vec()
				.into(),
			)
			.await
			.unwrap();

		let (sender, mut receiver) = rx.recv().timeout(Duration::from_secs(2)).await.unwrap().unwrap();

		assert_eq!(
			receiver.message().await.unwrap().unwrap().message.unwrap(),
			ingest_watch_request::Message::Open(ingest_watch_request::Open {
				request_id: Some(new_req_id.into()),
			})
		);

		let mut audio = None;
		let mut video = None;

		for packet in &packets {
			match packet {
				TransmuxResult::InitSegment {
					data,
					audio_settings,
					video_settings,
				} => {
					audio = Some(audio_settings);
					video = Some(video_settings);
					sender
						.send(Ok(IngestWatchResponse {
							message: Some(ingest_watch_response::Message::Media(ingest_watch_response::Media {
								data: data.clone(),
								keyframe: false,
								timescale: 0,
								timestamp: 0,
								r#type: ingest_watch_response::media::Type::Init.into(),
							})),
						}))
						.await
						.unwrap();

					sender
						.send(Ok(IngestWatchResponse {
							message: Some(ingest_watch_response::Message::Ready(
								ingest_watch_response::Ready::Ready.into(),
							)),
						}))
						.await
						.unwrap();
				}
				TransmuxResult::MediaSegment(ms) => {
					tokio::time::sleep(Duration::from_millis(10)).await;

					sender
						.send(Ok(IngestWatchResponse {
							message: Some(ingest_watch_response::Message::Media(ingest_watch_response::Media {
								data: ms.data.clone(),
								keyframe: ms.keyframe,
								timescale: match ms.ty {
									transmuxer::MediaType::Audio => audio.as_ref().unwrap().timescale,
									transmuxer::MediaType::Video => video.as_ref().unwrap().timescale,
								},
								timestamp: ms.timestamp,
								r#type: match ms.ty {
									transmuxer::MediaType::Audio => ingest_watch_response::media::Type::Audio.into(),
									transmuxer::MediaType::Video => ingest_watch_response::media::Type::Video.into(),
								},
							})),
						}))
						.await
						.unwrap();
				}
			}
		}

		{
			let event = Event::decode(event_stream.next().await.unwrap().payload).unwrap();
			assert!(event.timestamp - chrono::Utc::now().timestamp_millis() < 1000);
			match event.event {
				Some(event::Event::Room(room)) => {
					assert_eq!(room.room_id.into_ulid(), room_id);

					match room.event {
						Some(event::room::Event::Ready(ready)) => {
							assert_eq!(ready.connection_id.into_ulid(), connection_id);
						}
						_ => panic!("unexpected event"),
					}
				}
				_ => panic!("unexpected event"),
			};
		}

		sender
			.send(Ok(IngestWatchResponse {
				message: Some(ingest_watch_response::Message::Shutdown(
					ingest_watch_response::Shutdown::Stream.into(),
				)),
			}))
			.await
			.unwrap();
		assert!(receiver.message().await.unwrap().is_none());

		let video_manifest = LiveRenditionManifest::decode(
			global
				.metadata_store()
				.get(video_common::keys::rendition_manifest(
					org_id,
					room_id,
					connection_id,
					Rendition::VideoSource.into(),
				))
				.await
				.unwrap()
				.unwrap(),
		)
		.unwrap();
		let audio_manifest = LiveRenditionManifest::decode(
			global
				.metadata_store()
				.get(video_common::keys::rendition_manifest(
					org_id,
					room_id,
					connection_id,
					Rendition::AudioSource.into(),
				))
				.await
				.unwrap()
				.unwrap(),
		)
		.unwrap();

		assert_eq!(video_manifest.segments.len(), 4);
		assert_eq!(video_manifest.segments[0].parts.len(), 4);
		assert_eq!(video_manifest.segments[1].parts.len(), 4);
		assert_eq!(video_manifest.segments[2].parts.len(), 4);
		assert_eq!(video_manifest.segments[3].parts.len(), 4);
		assert_eq!(video_manifest.segments[0].parts.iter().filter(|p| p.independent).count(), 1);
		assert_eq!(video_manifest.segments[1].parts.iter().filter(|p| p.independent).count(), 1);
		assert_eq!(video_manifest.segments[2].parts.iter().filter(|p| p.independent).count(), 1);
		assert_eq!(video_manifest.segments[3].parts.iter().filter(|p| p.independent).count(), 1);
		assert!(video_manifest.segments[0].parts[0].independent);
		assert!(video_manifest.segments[1].parts[0].independent);
		assert!(video_manifest.segments[2].parts[0].independent);
		assert!(video_manifest.segments[3].parts[0].independent);
		assert!(video_manifest.completed);
		assert_eq!(video_manifest.info.as_ref().unwrap().next_segment_idx, 4);
		assert_eq!(video_manifest.info.as_ref().unwrap().next_part_idx, 16);
		assert_eq!(video_manifest.info.as_ref().unwrap().next_segment_part_idx, 0);
		assert_eq!(video_manifest.other_info["audio_source"].next_segment_idx, 4);
		assert_eq!(video_manifest.other_info["audio_source"].next_part_idx, 16);
		assert_eq!(video_manifest.other_info["audio_source"].next_segment_part_idx, 0);
		assert_eq!(video_manifest.total_duration, 59000 * 4); // verified with ffprobe

		assert_eq!(audio_manifest.segments.len(), 4);
		assert_eq!(audio_manifest.segments[0].parts.len(), 4);
		assert_eq!(audio_manifest.segments[1].parts.len(), 4);
		assert_eq!(audio_manifest.segments[2].parts.len(), 4);
		assert_eq!(audio_manifest.segments[3].parts.len(), 4);
		assert!(
			audio_manifest
				.segments
				.iter()
				.flat_map(|s| s.parts.iter())
				.all(|p| p.independent)
		);
		assert!(audio_manifest.completed);
		assert_eq!(audio_manifest.info.as_ref().unwrap().next_segment_idx, 4);
		assert_eq!(audio_manifest.info.as_ref().unwrap().next_part_idx, 16);
		assert_eq!(audio_manifest.info.as_ref().unwrap().next_segment_part_idx, 0);
		assert_eq!(audio_manifest.other_info["video_source"].next_segment_idx, 4);
		assert_eq!(audio_manifest.other_info["video_source"].next_part_idx, 16);
		assert_eq!(audio_manifest.other_info["video_source"].next_segment_part_idx, 0);
		assert_eq!(audio_manifest.total_duration, 48128 * 4); // verified with
		// ffprobe
	}

	drop(global);
	handler.cancel().timeout(Duration::from_secs(2)).await.unwrap();
	transcoder_run_handle
		.timeout(Duration::from_secs(2))
		.await
		.unwrap()
		.unwrap()
		.unwrap();

	tracing::info!("done");
}
