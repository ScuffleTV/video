use std::path::PathBuf;
use std::time::Duration;

use scuffle_utils::prelude::FutureTimeout;
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::channels::{ChannelData, UniqueID};
use crate::Session;

#[tokio::test]
async fn test_basic_rtmp_clean() {
	let listener = tokio::net::TcpListener::bind("0.0.0.0:0").await.expect("failed to bind");
	let addr = listener.local_addr().unwrap();

	let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");

	let mut ffmpeg = Command::new("ffmpeg")
		.args([
			"-re",
			"-i",
			dir.join("avc_aac.mp4").to_str().expect("failed to get path"),
			"-r",
			"30",
			"-t",
			"1", // just for the test so it doesn't take too long
			"-c",
			"copy",
			"-f",
			"flv",
			&format!("rtmp://{}:{}/live/stream-key", addr.ip(), addr.port()),
		])
		.stdout(std::process::Stdio::inherit())
		.stderr(std::process::Stdio::inherit())
		.spawn()
		.expect("failed to execute ffmpeg");

	let (ffmpeg_stream, _) = listener
		.accept()
		.timeout(Duration::from_millis(1000))
		.await
		.expect("timedout")
		.expect("failed to accept");

	let (ffmpeg_handle, mut ffmpeg_data_reciever, mut ffmpeg_event_reciever) = {
		let (ffmpeg_event_producer, ffmpeg_event_reciever) = mpsc::channel(1);
		let (ffmpeg_data_producer, ffmpeg_data_reciever) = mpsc::channel(128);
		let mut session = Session::new(ffmpeg_stream, ffmpeg_data_producer, ffmpeg_event_producer);

		(
			tokio::spawn(async move {
				let r = session.run().await;
				tracing::debug!("ffmpeg session ended: {:?}", r);
				r
			}),
			ffmpeg_data_reciever,
			ffmpeg_event_reciever,
		)
	};

	let event = ffmpeg_event_reciever
		.recv()
		.timeout(Duration::from_millis(1000))
		.await
		.expect("timedout")
		.expect("failed to recv event");

	assert_eq!(event.app_name, "live");
	assert_eq!(event.stream_name, "stream-key");

	let stream_id = UniqueID::new_v4();
	event.response.send(stream_id).expect("failed to send response");

	let mut got_video = false;
	let mut got_audio = false;
	let mut got_metadata = false;

	while let Some(data) = ffmpeg_data_reciever
		.recv()
		.timeout(Duration::from_millis(1000))
		.await
		.expect("timedout")
	{
		match data {
			ChannelData::Video { .. } => got_video = true,
			ChannelData::Audio { .. } => got_audio = true,
			ChannelData::Metadata { .. } => got_metadata = true,
		}
	}

	assert!(got_video);
	assert!(got_audio);
	assert!(got_metadata);

	assert!(
		ffmpeg_handle
			.await
			.expect("failed to join handle")
			.expect("failed to handle ffmpeg connection")
	);
	assert!(ffmpeg.try_wait().expect("failed to wait for ffmpeg").is_none());
}

#[tokio::test]
async fn test_basic_rtmp_unclean() {
	let listener = tokio::net::TcpListener::bind("0.0.0.0:0").await.expect("failed to bind");
	let addr = listener.local_addr().unwrap();

	let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");

	let mut ffmpeg = Command::new("ffmpeg")
		.args([
			"-re",
			"-i",
			dir.join("avc_aac.mp4").to_str().expect("failed to get path"),
			"-r",
			"30",
			"-t",
			"1", // just for the test so it doesn't take too long
			"-c",
			"copy",
			"-f",
			"flv",
			&format!("rtmp://{}:{}/live/stream-key", addr.ip(), addr.port()),
		])
		.stdout(std::process::Stdio::inherit())
		.stderr(std::process::Stdio::inherit())
		.spawn()
		.expect("failed to execute ffmpeg");

	let (ffmpeg_stream, _) = listener
		.accept()
		.timeout(Duration::from_millis(1000))
		.await
		.expect("timedout")
		.expect("failed to accept");

	let (ffmpeg_handle, mut ffmpeg_data_reciever, mut ffmpeg_event_reciever) = {
		let (ffmpeg_event_producer, ffmpeg_event_reciever) = mpsc::channel(1);
		let (ffmpeg_data_producer, ffmpeg_data_reciever) = mpsc::channel(128);
		let mut session = Session::new(ffmpeg_stream, ffmpeg_data_producer, ffmpeg_event_producer);

		(
			tokio::spawn(async move {
				let r = session.run().await;
				tracing::debug!("ffmpeg session ended: {:?}", r);
				r
			}),
			ffmpeg_data_reciever,
			ffmpeg_event_reciever,
		)
	};

	let event = ffmpeg_event_reciever
		.recv()
		.timeout(Duration::from_millis(1000))
		.await
		.expect("timedout")
		.expect("failed to recv event");

	assert_eq!(event.app_name, "live");
	assert_eq!(event.stream_name, "stream-key");

	let stream_id = UniqueID::new_v4();
	event.response.send(stream_id).expect("failed to send response");

	let mut got_video = false;
	let mut got_audio = false;
	let mut got_metadata = false;

	while let Some(data) = ffmpeg_data_reciever
		.recv()
		.timeout(Duration::from_millis(1000))
		.await
		.expect("timedout")
	{
		match data {
			ChannelData::Video { .. } => got_video = true,
			ChannelData::Audio { .. } => got_audio = true,
			ChannelData::Metadata { .. } => got_metadata = true,
		}

		if got_video && got_audio && got_metadata {
			break;
		}
	}

	assert!(got_video);
	assert!(got_audio);
	assert!(got_metadata);

	ffmpeg.kill().await.expect("failed to kill ffmpeg");

	// the server should have detected the ffmpeg process has died uncleanly
	assert!(
		!ffmpeg_handle
			.await
			.expect("failed to join handle")
			.expect("failed to handle ffmpeg connection")
	);
}
