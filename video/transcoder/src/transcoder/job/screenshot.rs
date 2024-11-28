use anyhow::Context;
use bytes::Bytes;
use image::codecs::jpeg::JpegEncoder;
use scuffle_ffmpeg::ffi::AVPixelFormat;
use scuffle_ffmpeg::frame::Frame;
use tokio::sync::mpsc;

pub fn screenshot_task(mut recv: mpsc::Receiver<Frame>, send: mpsc::Sender<(Bytes, f64)>) -> anyhow::Result<()> {
	while let Some(frame) = recv.blocking_recv() {
		let _guard = scuffle_utils::task::AbortGuard::new();

		let frame = frame.video();

		let mut writer = Vec::new();
		let mut encoder = JpegEncoder::new_with_quality(&mut writer, 95);
		let width = frame.width() as u32;
		let height = frame.height() as u32;
		let data = frame.data(0).ok_or_else(|| anyhow::anyhow!("no frame data"))?;

		if frame.format() != AVPixelFormat::AV_PIX_FMT_RGBA as i32 {
			anyhow::bail!("expected rgb frame");
		}

		encoder
			.encode(data, width, height, image::ExtendedColorType::Rgba8)
			.context("failed to encode jpeg")?;

		let data = Bytes::from(writer);

		let timestamp = frame
			.pts()
			.or_else(|| frame.best_effort_timestamp())
			.ok_or_else(|| anyhow::anyhow!("no frame timestamp"))?;

		let time = timestamp as f64 * frame.time_base().num as f64 / frame.time_base().den as f64;

		send.blocking_send((data, time)).context("failed to send screenshot")?;
	}

	Ok(())
}
