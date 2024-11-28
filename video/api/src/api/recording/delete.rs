use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use futures_util::StreamExt;
use pb::ext::UlidExt;
use pb::scuffle::video::internal::events::{recording_delete_batch_task, RecordingDeleteBatchTask};
use pb::scuffle::video::v1::types::access_token_scope::Permission;
use pb::scuffle::video::v1::types::{FailedResource, Resource};
use pb::scuffle::video::v1::{RecordingDeleteRequest, RecordingDeleteResponse};
use prost::Message;
use scuffle_utils::database::ClientLike;
use tonic::Status;
use ulid::Ulid;
use video_common::database::{AccessToken, DatabaseTable, Rendition};

use crate::api::utils::{impl_request_scopes, ApiRequest, TonicRequest};
use crate::config::ApiConfig;
use crate::global::ApiGlobal;
use crate::ratelimit::RateLimitResource;

impl_request_scopes!(
	RecordingDeleteRequest,
	video_common::database::Recording,
	(Resource::Recording, Permission::Delete),
	RateLimitResource::RecordingDelete
);

#[derive(postgres_from_row::FromRow)]
struct ThumbnailResp {
	recording_id: Ulid,
	id: Ulid,
	idx: i32,
}

#[derive(postgres_from_row::FromRow)]
struct SegmentResp {
	recording_id: Ulid,
	id: Ulid,
	idx: i32,
	rendition: Rendition,
}

#[derive(postgres_from_row::FromRow)]
struct RecordingResp {
	id: Ulid,
	s3_bucket_id: Ulid,
}

trait UpdateBatch {
	const NAME: &'static str;

	fn is_same_batch(&self, batch: &RecordingDeleteBatchTask) -> bool;
	fn update_batch(&self, deleted_recordings: &HashMap<Ulid, Ulid>, batch: &mut RecordingDeleteBatchTask);
	fn to_object(&self) -> recording_delete_batch_task::Object;
}

impl UpdateBatch for ThumbnailResp {
	const NAME: &'static str = "thumbnail";

	fn is_same_batch(&self, batch: &RecordingDeleteBatchTask) -> bool {
		batch.recording_id.into_ulid() == self.recording_id
			&& matches!(
				batch.objects_type,
				Some(recording_delete_batch_task::ObjectsType::Thumbnails(_))
			)
	}

	fn update_batch(&self, deleted_recordings: &HashMap<Ulid, Ulid>, batch: &mut RecordingDeleteBatchTask) {
		batch.recording_id = Some(self.recording_id.into());
		batch.s3_bucket_id = Some(deleted_recordings[&self.recording_id].into());
		batch.objects_type = Some(recording_delete_batch_task::ObjectsType::Thumbnails(
			recording_delete_batch_task::ThumbnailType {},
		));
		batch.objects.clear();
	}

	fn to_object(&self) -> recording_delete_batch_task::Object {
		recording_delete_batch_task::Object {
			index: self.idx,
			object_id: Some(self.id.into()),
		}
	}
}

impl UpdateBatch for SegmentResp {
	const NAME: &'static str = "segment";

	fn is_same_batch(&self, batch: &RecordingDeleteBatchTask) -> bool {
		batch.recording_id.into_ulid() == self.recording_id
			&& batch.objects_type
				== Some(recording_delete_batch_task::ObjectsType::Segments(
					pb::scuffle::video::v1::types::Rendition::from(self.rendition) as i32,
				))
	}

	fn update_batch(&self, deleted_recordings: &HashMap<Ulid, Ulid>, batch: &mut RecordingDeleteBatchTask) {
		batch.recording_id = Some(self.recording_id.into());
		batch.s3_bucket_id = Some(deleted_recordings[&self.recording_id].into());
		batch.objects_type = Some(recording_delete_batch_task::ObjectsType::Segments(
			pb::scuffle::video::v1::types::Rendition::from(self.rendition) as i32,
		));
		batch.objects.clear();
	}

	fn to_object(&self) -> recording_delete_batch_task::Object {
		recording_delete_batch_task::Object {
			index: self.idx,
			object_id: Some(self.id.into()),
		}
	}
}

async fn handle_resp(
	global: &Arc<impl ApiGlobal>,
	deleted_recordings: &HashMap<Ulid, Ulid>,
	resp: impl UpdateBatch,
	batch: &mut RecordingDeleteBatchTask,
) -> Option<()> {
	if !resp.is_same_batch(batch) || batch.objects.len() >= global.config::<ApiConfig>().recording_delete_batch_size {
		publish_batch(global, batch).await?;
		resp.update_batch(deleted_recordings, batch);
	}

	batch.objects.push(resp.to_object());

	Some(())
}

async fn publish_batch(global: &Arc<impl ApiGlobal>, batch: &RecordingDeleteBatchTask) -> Option<()> {
	if !batch.objects.is_empty() {
		global
			.nats()
			.publish(
				global.config::<ApiConfig>().recording_delete_stream.clone(),
				batch.encode_to_vec().into(),
			)
			.await
			.map_err(|err| {
				tracing::error!(err = %err, "failed to publish recording delete batch");
			})
			.ok()?;
	}

	Some(())
}

async fn handle_end_of_stream(global: &Arc<impl ApiGlobal>, batch: &mut RecordingDeleteBatchTask) -> Option<()> {
	publish_batch(global, batch).await?;

	// Reset the batch
	batch.recording_id = None;
	batch.s3_bucket_id = None;
	batch.objects_type = None;
	batch.objects.clear();

	Some(())
}

async fn handle_query<B>(
	global: &Arc<impl ApiGlobal>,
	client: impl ClientLike,
	deleted_recordings: &HashMap<Ulid, Ulid>,
	batch: &mut RecordingDeleteBatchTask,
	qb: &mut scuffle_utils::database::QueryBuilder<'_>,
) -> Option<()>
where
	B: UpdateBatch + postgres_from_row::FromRow + Send + Unpin,
{
	let mut qb = qb
		.build_query_as::<B>()
		.fetch_many(client)
		.await
		.map_err(|err| {
			tracing::error!(err = %err, "failed to fetch recording {}s", B::NAME);
		})
		.ok()?;

	let mut qb = std::pin::pin!(qb);

	while let Some(result) = qb.next().await {
		let result = result
			.map_err(|err| {
				tracing::error!(err = %err, "failed to fetch recording {}s", B::NAME);
			})
			.ok()?;

		handle_resp(global, deleted_recordings, result, batch).await?;
	}

	Some(())
}

impl ApiRequest<RecordingDeleteResponse> for tonic::Request<RecordingDeleteRequest> {
	async fn process<G: ApiGlobal>(
		&self,
		global: &Arc<G>,
		access_token: &AccessToken,
	) -> tonic::Result<tonic::Response<RecordingDeleteResponse>> {
		let req = self.get_ref();

		if req.ids.len() > 100 {
			return Err(tonic::Status::invalid_argument(
				"too many ids provided for delete: max 100".to_string(),
			));
		}

		if req.ids.is_empty() {
			return Err(tonic::Status::invalid_argument("no ids provided for delete"));
		}

		let mut ids_to_delete = req
			.ids
			.iter()
			.copied()
			.map(pb::scuffle::types::Ulid::into_ulid)
			.collect::<HashSet<_>>();

		let mut client = global.db().get().await.map_err(|err| {
			tracing::error!(err = %err, "failed to get db client");
			Status::internal("internal server error")
		})?;

		let tx = client.transaction().await.map_err(|e| {
			tracing::error!(err = %e, "beginning transaction");
			tonic::Status::internal("playback session revoke failed")
		})?;

		// We dont actually want to delete the recordings from the database, we just
		// want to mark them as deleted
		let deleted_recordings: Vec<RecordingResp> = scuffle_utils::database::query("UPDATE ")
			.push(<RecordingDeleteRequest as TonicRequest>::Table::NAME)
			.push(" SET deleted_at = NOW(), room_id = NULL, recording_config_id = NULL")
			.push(" WHERE id = ANY(")
			.push_bind(ids_to_delete.iter().copied().collect::<Vec<_>>())
			.push(") AND organization_id = ")
			.push_bind(access_token.organization_id)
			.push(" AND deleted_at IS NULL")
			.push(" RETURNING id, s3_bucket_id")
			.build_query_as()
			.fetch_all(&tx)
			.await
			.map_err(|err| {
				tracing::error!(err = %err, "failed to update {}s", <RecordingDeleteRequest as TonicRequest>::Table::FRIENDLY_NAME);
				tonic::Status::internal(format!(
					"failed to delete {}s",
					<RecordingDeleteRequest as TonicRequest>::Table::FRIENDLY_NAME
				))
			})?;

		let deleted_ids = deleted_recordings.iter().map(|resp| resp.id).collect::<Vec<_>>();

		let deleted_recordings = deleted_recordings
			.into_iter()
			.map(|resp| (resp.id, resp.s3_bucket_id))
			.collect::<HashMap<_, _>>();

		deleted_ids.iter().for_each(|id| {
			ids_to_delete.remove(id);
		});

		scuffle_utils::database::query("DELETE FROM ")
			.push(<video_common::database::PlaybackSession as DatabaseTable>::NAME)
			.push(" WHERE recording_id = ANY(")
			.push_bind(&deleted_ids)
			.push(") AND organization_id = ")
			.push_bind(access_token.organization_id)
			.build().execute(&tx).await.map_err(|err| {
			tracing::error!(err = %err, "failed to delete {}s", <video_common::database::PlaybackSession as DatabaseTable>::FRIENDLY_NAME);
			tonic::Status::internal(format!("failed to delete {}s, the recording have not been deleted", <video_common::database::PlaybackSession as DatabaseTable>::FRIENDLY_NAME))
		})?;

		scuffle_utils::database::query("DELETE FROM ")
			.push(<video_common::database::RecordingRendition as DatabaseTable>::NAME)
			.push(" WHERE recording_id = ANY(")
			.push_bind(&deleted_ids)
			.push(") AND organization_id = ")
			.push_bind(access_token.organization_id)
			.build().execute(&tx).await.map_err(|err| {
			tracing::error!(err = %err, "failed to delete {}s", <video_common::database::PlaybackSession as DatabaseTable>::FRIENDLY_NAME);
			tonic::Status::internal(format!("failed to delete {}s, the recording have not been deleted", <video_common::database::PlaybackSession as DatabaseTable>::FRIENDLY_NAME))
		})?;

		tx.commit().await.map_err(|err| {
			tracing::error!(err = %err, "failed to commit transaction");
			tonic::Status::internal("failed to commit transaction, the recording have not been deleted")
		})?;

		// The next part is resource cleanup in S3. Regardless if this next part fails
		// we can detect these failures At the database state level and retry the
		// cleanup later.

		let allowed_to_fail = || async {
			let mut batch = RecordingDeleteBatchTask {
				recording_id: None,
				s3_bucket_id: None,
				objects_type: None,
				objects: Vec::with_capacity(global.config::<ApiConfig>().recording_delete_batch_size),
			};

			handle_query::<ThumbnailResp>(
				global,
				&client,
				&deleted_recordings,
				&mut batch,
				scuffle_utils::database::query("SELECT id, recording_id, idx FROM ")
					.push(<video_common::database::RecordingThumbnail as DatabaseTable>::NAME)
					.push(" WHERE recording_id = ANY(")
					.push_bind(&deleted_ids)
					.push(") AND organization_id = ")
					.push_bind(access_token.organization_id)
					.push(" ORDER BY recording_id"),
			)
			.await?;

			handle_end_of_stream(global, &mut batch).await?;

			handle_query::<SegmentResp>(
				global,
				&client,
				&deleted_recordings,
				&mut batch,
				scuffle_utils::database::query("SELECT id, recording_id, rendition, idx FROM ")
					.push(<video_common::database::RecordingRenditionSegment as DatabaseTable>::NAME)
					.push(" WHERE recording_id = ANY(")
					.push_bind(&deleted_ids)
					.push(") ")
					.push(" AND organization_id = ")
					.push_bind(access_token.organization_id)
					.push(" ORDER BY recording_id, rendition"),
			)
			.await?;

			handle_end_of_stream(global, &mut batch).await
		};

		allowed_to_fail().await;

		Ok(tonic::Response::new(RecordingDeleteResponse {
			ids: deleted_ids.into_iter().map(|id| id.into()).collect(),
			failed_deletes: ids_to_delete
				.into_iter()
				.map(|id| FailedResource {
					id: Some(id.into()),
					reason: "recording not found".into(),
				})
				.collect(),
		}))
	}
}
