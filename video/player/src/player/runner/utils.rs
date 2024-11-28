use tokio::sync::mpsc;
use web_sys::{HtmlVideoElement, MediaSource};

use crate::player::util::{register_events, Holder};

pub enum VideoElementEvent {
	Error(web_sys::ErrorEvent),
	Waiting,
	Pause,
	Play,
	Playing,
	Seeking,
	TimeUpdate,
}

pub fn make_video_element_holder(element: HtmlVideoElement) -> Holder<HtmlVideoElement, VideoElementEvent> {
	let (tx, el_rx) = mpsc::channel(128);

	let cleanup = register_events!(element, {
		"error" => {
			let tx = tx.clone();
			move |evt: web_sys::Event| {
				tracing::error!("video element error");
				if tx.try_send(VideoElementEvent::Error(evt.unchecked_into())).is_err() {
					tracing::warn!("failed to send error event");
				}
			}
		},
		"waiting" => {
			let tx = tx.clone();
			move |_| {
				if tx.try_send(VideoElementEvent::Waiting).is_err() {
					tracing::warn!("failed to send waiting event");
				}
			}
		},
		"playing" => {
			let tx = tx.clone();
			move |_| {
				if tx.try_send(VideoElementEvent::Playing).is_err() {
					tracing::warn!("failed to send playing event");
				}
			}
		},
		"pause" => {
			let tx = tx.clone();
			move |_| {
				if tx.try_send(VideoElementEvent::Pause).is_err() {
					tracing::warn!("failed to send pause event");
				}
			}
		},
		"play" => {
			let tx = tx.clone();
			move |_| {
				if tx.try_send(VideoElementEvent::Play).is_err() {
					tracing::warn!("failed to send play event");
				}
			}
		},
		"seeking" => {
			let tx = tx.clone();
			move |_| {
				if tx.try_send(VideoElementEvent::Seeking).is_err() {
					tracing::warn!("failed to send seeking event");
				}
			}
		},
		"timeupdate" => {
			let tx = tx.clone();
			move |_| {
				if tx.try_send(VideoElementEvent::TimeUpdate).is_err() {
					tracing::warn!("failed to send timeupdate event");
				}
			}
		}
	});

	Holder::new(element, el_rx, cleanup)
}

pub enum MediaSourceEvent {
	SourceOpen,
	SourceClose,
}

pub fn make_media_source_holder() -> Holder<MediaSource, MediaSourceEvent> {
	let mediasource = MediaSource::new().unwrap();
	let (tx, mediasource_rx) = mpsc::channel(8);

	let cleanup = register_events!(mediasource, {
		"sourceopen" => {
			let tx = tx.clone();
			move |_| {
				if tx.try_send(MediaSourceEvent::SourceOpen).is_err() {
					tracing::warn!("failed to send sourceopen event");
				}
			}
		},
		"sourceclose" => {
			let tx = tx.clone();
			move |_| {
				if tx.try_send(MediaSourceEvent::SourceClose).is_err() {
					tracing::warn!("failed to send sourceclose event");
				}
			}
		},
	});

	Holder::new(mediasource, mediasource_rx, cleanup)
}
