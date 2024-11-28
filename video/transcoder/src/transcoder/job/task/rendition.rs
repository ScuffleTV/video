use std::sync::Arc;

use anyhow::Context;
use bytes::Bytes;
use tokio::sync::mpsc;
use ulid::Ulid;
use video_common::database::Rendition;

use super::retry_task;
use crate::global::TranscoderGlobal;

#[derive(Clone)]
pub enum TrackTask {
	Init { data: Bytes },
	Media { part_idx: u32, data: Bytes },
	Manifest { data: Bytes },
}

pub async fn track_task(
	global: Arc<impl TranscoderGlobal>,
	organization_id: Ulid,
	room_id: Ulid,
	connection_id: Ulid,
	rendition: Rendition,
	mut rx: mpsc::Receiver<TrackTask>,
) -> anyhow::Result<()> {
	while let Some(task) = rx.recv().await {
		retry_task(
			|| async {
				match &task {
					TrackTask::Init { data } => {
						let key = video_common::keys::init(organization_id, room_id, connection_id, rendition);
						global
							.media_store()
							.put(key.as_str(), &mut std::io::Cursor::new(data))
							.await
							.context("upload init")?;
					}
					TrackTask::Media { part_idx, data } => {
						let key = video_common::keys::part(organization_id, room_id, connection_id, rendition, *part_idx);
						global
							.media_store()
							.put(key.as_str(), &mut std::io::Cursor::new(data))
							.await
							.context("upload part")?;
					}
					TrackTask::Manifest { data } => {
						let key = video_common::keys::rendition_manifest(organization_id, room_id, connection_id, rendition);
						global
							.metadata_store()
							.put(key.as_str(), data.clone())
							.await
							.context("upload manifest")?;
					}
				}

				Ok(())
			},
			5,
		)
		.await
		.context("rendition_task")?;
	}

	Ok(())
}
