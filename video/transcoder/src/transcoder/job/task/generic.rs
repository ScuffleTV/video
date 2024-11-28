use std::sync::Arc;

use anyhow::Context;
use bytes::Bytes;
use pb::scuffle::video::v1::events_fetch_request::Target;
use pb::scuffle::video::v1::types::event;
use tokio::sync::mpsc;
use ulid::Ulid;
use video_common::database::RoomStatus;

use super::retry_task;
use crate::global::TranscoderGlobal;

pub enum GenericTask {
	Screenshot { data: Bytes, idx: u32 },
	Manifest { data: Bytes },
	RoomReady,
}

pub async fn generic_task(
	global: Arc<impl TranscoderGlobal>,
	organization_id: Ulid,
	room_id: Ulid,
	connection_id: Ulid,
	mut rx: mpsc::Receiver<GenericTask>,
) -> anyhow::Result<()> {
	while let Some(task) = rx.recv().await {
		retry_task(
			|| async {
				match &task {
					GenericTask::Screenshot { data, idx } => {
						let key = video_common::keys::screenshot(organization_id, room_id, connection_id, *idx);
						global
							.media_store()
							.put(key.as_str(), &mut std::io::Cursor::new(&data))
							.await
							.context("upload screenshot")?;
					}
					GenericTask::Manifest { data } => {
						let key = video_common::keys::manifest(organization_id, room_id, connection_id);
						global
							.metadata_store()
							.put(key.as_str(), data.clone())
							.await
							.context("upload manifest")?;
					}
					GenericTask::RoomReady {} => {
						if scuffle_utils::database::query(
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
						.bind(RoomStatus::Ready)
						.bind(organization_id)
						.bind(room_id)
						.bind(connection_id)
						.build()
						.execute(global.db())
						.await
						.context("update room status")?
							!= 1
						{
							anyhow::bail!("failed to update room status");
						};

						video_common::events::emit(
							global.nats(),
							&global.config().events_stream_name,
							organization_id,
							Target::Room,
							event::Event::Room(event::Room {
								room_id: Some(room_id.into()),
								event: Some(event::room::Event::Ready(event::room::Ready {
									connection_id: Some(connection_id.into()),
								})),
							}),
						)
						.await;
					}
				}
				Ok(())
			},
			5,
		)
		.await
		.context("generic_task")?;
	}

	Ok(())
}
