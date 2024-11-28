use anyhow::Context;
use tokio::sync::mpsc;
use video_common::database::Rendition;

use crate::transcoder::job::track::parser::{TrackOut, TrackParser};

pub async fn track_parser_task(
	mut tp: TrackParser,
	rendition: Rendition,
	tx: mpsc::Sender<(Rendition, TrackOut)>,
) -> anyhow::Result<()> {
	while let Some(track) = tp.parse().await.context("track parser failed")? {
		tx.send((rendition, track)).await.context("output failed")?;
	}

	Ok(())
}
