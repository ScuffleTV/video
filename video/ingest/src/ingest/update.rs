use std::sync::Arc;
use std::time::Duration;

use scuffle_utils::prelude::FutureTimeout;
use tokio::sync::mpsc;
use ulid::Ulid;

use crate::global::IngestGlobal;

pub struct Update {
	pub bitrate: i64,
}

pub async fn update_db<G: IngestGlobal>(
	global: Arc<G>,
	id: Ulid,
	organization_id: Ulid,
	room_id: Ulid,
	mut update_reciever: mpsc::Receiver<Update>,
) {
	while let Some(update) = update_reciever.recv().await {
		let mut success = false;

		for _ in 0..5 {
			match scuffle_utils::database::query(
				r#"
                UPDATE rooms
                SET
                    updated_at = NOW(),
                    ingest_bitrate = $1
                WHERE
                    organization_id = $2 AND
                    id = $3 AND
                    active_ingest_connection_id = $4
                "#,
			)
			.bind(update.bitrate)
			.bind(organization_id)
			.bind(room_id)
			.bind(id)
			.build()
			.execute(global.db())
			.timeout(Duration::from_secs(3))
			.await
			{
				Ok(Ok(r)) => {
					if r != 1 {
						tracing::error!("failed to update api with bitrate - no rows affected");
						return;
					} else {
						success = true;
						break;
					}
				}
				Ok(Err(e)) => {
					tracing::error!(error = %e, "failed to update api with bitrate");
				}
				Err(_) => {
					tracing::error!("failed to update api with bitrate timedout");
				}
			}

			tokio::time::sleep(Duration::from_secs(1)).await;
		}

		if !success {
			tracing::error!("failed to update api with bitrate after 5 retries - giving up");
			return;
		}
	}
}
