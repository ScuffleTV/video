use std::sync::Arc;

use scuffle_utilsdataloader::{DataLoader, Loader, LoaderOutput};
use ulid::Ulid;

pub struct RoomLoader {
	db: Arc<utils::database::Pool>,
}

impl RoomLoader {
	pub fn new(db: Arc<utils::database::Pool>) -> DataLoader<Self> {
		DataLoader::new(Self { db })
	}
}

impl Loader for RoomLoader {
	type Error = ();
	type Key = (Ulid, Ulid);
	type Value = video_common::database::Room;

	async fn load(&self, keys: &[Self::Key]) -> LoaderOutput<Self> {
		let results: Vec<Self::Value> =
			scuffle_utils::database::query("SELECT * FROM rooms WHERE (organization_id, id) IN ")
				.push_tuples(keys, |mut qb, (organization_id, room_id)| {
					qb.push_bind(organization_id).push_bind(room_id);
				})
				.build_query_as()
				.fetch_all(&self.db)
				.await
				.map_err(|err| {
					tracing::error!(error = %err, "failed to load rooms");
				})?;

		Ok(results.into_iter().map(|v| ((v.organization_id, v.id), v)).collect())
	}
}
