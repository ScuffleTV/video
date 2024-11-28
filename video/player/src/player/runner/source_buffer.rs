use tokio::sync::mpsc;
use wasm_bindgen::JsValue;
use web_sys::{MediaSource, SourceBuffer, SourceBufferAppendMode, TimeRanges};

use crate::player::util::{register_events, Holder};

pub struct SourceBufferHolder {
	sb: Holder<SourceBuffer, ()>,
}

impl SourceBufferHolder {
	pub fn new(media_source: &MediaSource, codec: &str) -> Result<Self, JsValue> {
		let sb = media_source.add_source_buffer(codec)?;
		let (tx, rx) = mpsc::channel(1);

		let cleanup = register_events!(sb, {
			"updateend" => move |_| {
				tx.try_send(()).ok();
			}
		});

		sb.set_mode(SourceBufferAppendMode::Segments);

		Ok(Self {
			sb: Holder::new(sb, rx, cleanup),
		})
	}

	pub fn buffered(&self) -> Result<TimeRanges, JsValue> {
		self.sb.buffered()
	}

	async fn wait_update(&mut self) {
		while self.sb.updating() {
			self.sb.events().recv().await;
		}
	}

	pub async fn change_type(&mut self, codec: &str) -> Result<(), JsValue> {
		self.wait_update().await;
		self.sb.change_type(codec)?;
		Ok(())
	}

	pub async fn append_buffer(&mut self, mut data: Vec<u8>) -> Result<(), JsValue> {
		self.wait_update().await;
		self.sb.append_buffer_with_u8_array(data.as_mut_slice())?;
		Ok(())
	}

	pub async fn remove(&mut self, start: f64, end: f64) -> Result<(), JsValue> {
		if start >= end {
			return Ok(());
		}

		self.wait_update().await;
		self.sb.remove(start, end)?;
		Ok(())
	}
}

pub struct SourceBuffers {
	pub audio: SourceBufferHolder,
	pub video: SourceBufferHolder,
}

impl SourceBuffers {
	pub fn new(media_source: &MediaSource) -> Self {
		Self {
			audio: SourceBufferHolder::new(media_source, "audio/mp4;codecs=\"mp4a.40.2\"").unwrap(),
			video: SourceBufferHolder::new(media_source, "video/mp4;codecs=\"avc1.4d002a\"").unwrap(),
		}
	}
}
