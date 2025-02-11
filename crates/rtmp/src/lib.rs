#![cfg_attr(all(coverage_nightly, test), feature(coverage_attribute))]

mod chunk;
mod handshake;
mod messages;
mod netconnection;
mod netstream;
mod protocol_control_messages;
mod session;
mod user_control_messages;

pub use session::{Session, SessionData, SessionError, SessionHandler};

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

    use scuffle_future_ext::FutureExt;
    use tokio::process::Command;
    use tokio::sync::{mpsc, oneshot};

    use crate::session::{SessionData, SessionHandler};
    use crate::{Session, SessionError};

    enum Event {
        Publish {
            stream_id: u32,
            app_name: String,
            stream_name: String,
            response: oneshot::Sender<Result<(), SessionError>>,
        },
        Unpublish {
            stream_id: u32,
            response: oneshot::Sender<Result<(), SessionError>>,
        },
        Data {
            stream_id: u32,
            data: SessionData,
            response: oneshot::Sender<Result<(), SessionError>>,
        },
    }

    struct Handler(mpsc::Sender<Event>);

    impl SessionHandler for Handler {
        async fn on_publish(&self, stream_id: u32, app_name: &str, stream_name: &str) -> Result<(), SessionError> {
            let (response, reciever) = oneshot::channel();

            self.0
                .send(Event::Publish {
                    stream_id,
                    app_name: app_name.to_string(),
                    stream_name: stream_name.to_string(),
                    response,
                })
                .await
                .unwrap();

            reciever.await.unwrap()
        }

        async fn on_unpublish(&self, stream_id: u32) -> Result<(), SessionError> {
            let (response, reciever) = oneshot::channel();

            self.0.send(Event::Unpublish { stream_id, response }).await.unwrap();

            reciever.await.unwrap()
        }

        async fn on_data(&self, stream_id: u32, data: SessionData) -> Result<(), SessionError> {
            let (response, reciever) = oneshot::channel();
            self.0
                .send(Event::Data {
                    stream_id,
                    data,
                    response,
                })
                .await
                .unwrap();

            reciever.await.unwrap()
        }
    }

    #[cfg(not(valgrind))] // test is time-sensitive, consider refactoring?
    #[tokio::test]
    async fn test_basic_rtmp_clean() {
        let listener = tokio::net::TcpListener::bind("0.0.0.0:0").await.expect("failed to bind");
        let addr = listener.local_addr().unwrap();

        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");

        let _ffmpeg = Command::new("ffmpeg")
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
            .with_timeout(Duration::from_millis(1000))
            .await
            .expect("timedout")
            .expect("failed to accept");

        let (ffmpeg_handle, mut ffmpeg_event_reciever) = {
            let (ffmpeg_event_producer, ffmpeg_event_reciever) = mpsc::channel(1);
            let mut session = Session::new(ffmpeg_stream, Handler(ffmpeg_event_producer));

            (
                tokio::spawn(async move {
                    let r = session.run().await;
                    tracing::debug!("ffmpeg session ended: {:?}", r);
                    r
                }),
                ffmpeg_event_reciever,
            )
        };

        let event = ffmpeg_event_reciever
            .recv()
            .with_timeout(Duration::from_millis(1000))
            .await
            .expect("timedout")
            .expect("failed to recv event");

        match event {
            Event::Publish {
                stream_id,
                app_name,
                stream_name,
                response,
            } => {
                assert_eq!(stream_id, 1);
                assert_eq!(app_name, "live");
                assert_eq!(stream_name, "stream-key");
                response.send(Ok(())).expect("failed to send response");
            }
            _ => panic!("unexpected event"),
        }

        let mut got_video = false;
        let mut got_audio = false;
        let mut got_metadata = false;

        while let Some(data) = ffmpeg_event_reciever
            .recv()
            .with_timeout(Duration::from_millis(1000))
            .await
            .expect("timedout")
        {
            match data {
                Event::Data {
                    stream_id,
                    response,
                    data,
                    ..
                } => {
                    match data {
                        SessionData::Video { .. } => got_video = true,
                        SessionData::Audio { .. } => got_audio = true,
                        SessionData::Amf0 { .. } => got_metadata = true,
                    }
                    response.send(Ok(())).expect("failed to send response");
                    assert_eq!(stream_id, 1);
                }
                Event::Unpublish { stream_id, response } => {
                    assert_eq!(stream_id, 1);
                    response.send(Ok(())).expect("failed to send response");
                    break;
                }
                _ => panic!("unexpected event"),
            }
        }

        assert!(got_video);
        assert!(got_audio);
        assert!(got_metadata);

        if ffmpeg_event_reciever
            .recv()
            .with_timeout(Duration::from_millis(1000))
            .await
            .expect("timedout")
            .is_some()
        {
            panic!("unexpected event");
        }

        assert!(ffmpeg_handle
            .await
            .expect("failed to join handle")
            .expect("failed to handle ffmpeg connection"));

        // TODO: Fix this assertion
        // assert!(ffmpeg.try_wait().expect("failed to wait for ffmpeg").is_none());
    }

    #[cfg(not(valgrind))] // test is time-sensitive, consider refactoring?
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
            .with_timeout(Duration::from_millis(1000))
            .await
            .expect("timedout")
            .expect("failed to accept");

        let (ffmpeg_handle, mut ffmpeg_event_reciever) = {
            let (ffmpeg_event_producer, ffmpeg_event_reciever) = mpsc::channel(1);
            let mut session = Session::new(ffmpeg_stream, Handler(ffmpeg_event_producer));

            (
                tokio::spawn(async move {
                    let r = session.run().await;
                    tracing::debug!("ffmpeg session ended: {:?}", r);
                    r
                }),
                ffmpeg_event_reciever,
            )
        };

        let event = ffmpeg_event_reciever
            .recv()
            .with_timeout(Duration::from_millis(1000))
            .await
            .expect("timedout")
            .expect("failed to recv event");

        match event {
            Event::Publish {
                stream_id,
                app_name,
                stream_name,
                response,
            } => {
                assert_eq!(stream_id, 1);
                assert_eq!(app_name, "live");
                assert_eq!(stream_name, "stream-key");
                response.send(Ok(())).expect("failed to send response");
            }
            _ => panic!("unexpected event"),
        }

        let mut got_video = false;
        let mut got_audio = false;
        let mut got_metadata = false;

        while let Some(data) = ffmpeg_event_reciever
            .recv()
            .with_timeout(Duration::from_millis(1000))
            .await
            .expect("timedout")
        {
            match data {
                Event::Data {
                    stream_id,
                    response,
                    data,
                    ..
                } => {
                    assert_eq!(stream_id, 1);
                    match data {
                        SessionData::Video { .. } => got_video = true,
                        SessionData::Audio { .. } => got_audio = true,
                        SessionData::Amf0 { .. } => got_metadata = true,
                    }
                    response.send(Ok(())).expect("failed to send response");
                }
                _ => panic!("unexpected event"),
            }

            if got_video && got_audio && got_metadata {
                break;
            }
        }

        assert!(got_video);
        assert!(got_audio);
        assert!(got_metadata);

        ffmpeg.kill().await.expect("failed to kill ffmpeg");

        while let Some(data) = ffmpeg_event_reciever
            .recv()
            .with_timeout(Duration::from_millis(1000))
            .await
            .expect("timedout")
        {
            match data {
                Event::Data { response, .. } => {
                    response.send(Ok(())).expect("failed to send response");
                }
                _ => panic!("unexpected event"),
            }
        }

        // the server should have detected the ffmpeg process has died uncleanly
        assert!(!ffmpeg_handle
            .await
            .expect("failed to join handle")
            .expect("failed to handle ffmpeg connection"));
    }
}
