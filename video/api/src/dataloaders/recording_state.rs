use std::sync::Arc;

use itertools::Itertools;
use scuffle_utilsdataloader::{DataLoader, Loader, LoaderOutput};
use ulid::Ulid;
use video_common::database::{Recording, Rendition};

pub struct RecordingStateLoader {
	db: Arc<utils::database::Pool>,
}

impl RecordingStateLoader {
	pub fn new(db: Arc<utils::database::Pool>) -> DataLoader<Self> {
		DataLoader::new(Self { db })
	}
}

#[derive(Debug, Clone, Default)]
pub struct RecordingState(Vec<RecordingRenditionState>);

impl RecordingState {
	pub fn recording_to_proto(&self, recording: Recording) -> pb::scuffle::video::v1::types::Recording {
		let (size_bytes, start_time, end_time, renditions) =
			self.0
				.iter()
				.fold((0, 0.0, 0.0, Vec::new()), |(size_bytes, min, max, mut renditions), state| {
					renditions.push(state.rendition);
					(
						size_bytes + state.size_bytes,
						f32::min(min, state.start_time),
						f32::max(max, state.end_time),
						renditions,
					)
				});

		recording.into_proto(renditions, size_bytes, end_time - start_time)
	}
}

#[derive(Debug, Clone, postgres_from_row::FromRow)]
pub struct RecordingRenditionState {
	pub organization_id: Ulid,
	pub recording_id: Ulid,
	pub rendition: Rendition,
	pub size_bytes: i64,
	pub end_time: f32,
	pub start_time: f32,
}

impl Loader for RecordingStateLoader {
	type Error = ();
	type Key = (Ulid, Ulid);
	type Value = RecordingState;

	async fn load(&self, keys: &[Self::Key]) -> LoaderOutput<Self> {
		let results: Vec<RecordingRenditionState> = scuffle_utils::database::query("SELECT organization_id, recording_id, rendition, COUNT(size_bytes) AS size_bytes, MAX(end_time) AS end_time, MAX(start_time) AS start_time FROM recording_rendition_segments WHERE (organization_id, recording_id) IN ")
			.push_tuples(keys, |mut qb, (organization_id, recording_id)| {
			qb.push_bind(organization_id).push_bind(recording_id);
		}).push(" GROUP BY organization_id, recording_id, rendition ORDER BY organization_id, recording_id").build_query_as().fetch_all(&self.db).await.map_err(|err| {
			tracing::error!(error = %err, "failed to load access tokens");
		})?;

		Ok(results
			.into_iter()
			.group_by(|v| (v.organization_id, v.recording_id))
			.into_iter()
			.map(|(k, v)| (k, RecordingState(v.collect())))
			.collect())
	}
}
