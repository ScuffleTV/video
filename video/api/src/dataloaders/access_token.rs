use std::sync::Arc;

use scuffle_utilsdataloader::{DataLoader, Loader, LoaderOutput};
use ulid::Ulid;

pub struct AccessTokenLoader {
	db: Arc<utils::database::Pool>,
}

impl AccessTokenLoader {
	pub fn new(db: Arc<utils::database::Pool>) -> DataLoader<Self> {
		DataLoader::new(Self { db })
	}
}

impl Loader for AccessTokenLoader {
	type Error = ();
	type Key = (Ulid, Ulid);
	type Value = video_common::database::AccessToken;

	async fn load(&self, keys: &[Self::Key]) -> LoaderOutput<Self> {
		let results: Vec<Self::Value> =
			scuffle_utils::database::query("SELECT * FROM access_tokens WHERE (organization_id, id) IN ")
				.push_tuples(keys, |mut qb, (organization_id, access_token_id)| {
					qb.push_bind(organization_id).push_bind(access_token_id);
				})
				.build_query_as()
				.fetch_all(&self.db)
				.await
				.map_err(|err| {
					tracing::error!(error = %err, "failed to load access tokens");
				})?;

		Ok(results.into_iter().map(|v| ((v.organization_id, v.id), v)).collect())
	}
}
