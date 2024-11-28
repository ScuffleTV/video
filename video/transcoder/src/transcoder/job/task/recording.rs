use std::sync::Arc;

use anyhow::Context;
use aws_sdk_s3::types::ObjectCannedAcl;
use binary_helper::s3::{AsyncStreamBody, PutObjectOptions};
use bytes::Bytes;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use ulid::Ulid;
use video_common::database::Rendition;

use super::retry_task;
use crate::global::TranscoderGlobal;

pub enum RecordingTask {
	Segment {
		segment_id: Ulid,
		segment_idx: u32,
		duration: f64,
		start_time: f64,
		parts: Vec<Bytes>,
	},
	Init {
		data: Bytes,
	},
}

#[inline(always)]
fn normalize_float(f: f64) -> f64 {
	(f * 1000.0).round() / 1000.0
}

pub async fn recording_task(
	global: Arc<impl TranscoderGlobal>,
	organization_id: Ulid,
	recording_id: Ulid,
	rendition: Rendition,
	bucket: binary_helper::s3::Bucket,
	mut rx: mpsc::Receiver<RecordingTask>,
) -> anyhow::Result<()> {
	while let Some(task) = rx.recv().await {
		retry_task(
			|| async {
				match &task {
					RecordingTask::Segment {
						segment_id,
						segment_idx,
						duration,
						start_time,
						parts,
					} => {
						let size = parts.iter().map(|p| p.len()).sum::<usize>();

						let stream = futures_util::stream::iter(parts.clone()).map(std::io::Result::Ok);

						let segment = video_common::keys::s3_segment(
							organization_id,
							recording_id,
							rendition,
							*segment_idx,
							*segment_id,
						);

						bucket
							.put_object(
								segment,
								AsyncStreamBody(stream),
								Some(PutObjectOptions {
									content_type: Some("video/mp4".to_owned()),
									acl: Some(ObjectCannedAcl::PublicRead),
								}),
							)
							.await
							.context("upload segment")?;

						if scuffle_utils::database::query(
							r#"
                        INSERT INTO recording_rendition_segments (
                            organization_id,
                            recording_id,
                            rendition,
                            idx,
                            id,
                            start_time,
                            end_time,
                            size_bytes
                        ) VALUES (
                            $1,
                            $2,
                            $3,
                            $4,
                            $5,
                            $6,
                            $7,
                            $8
                        )"#,
						)
						.bind(organization_id)
						.bind(recording_id)
						.bind(rendition)
						.bind(*segment_idx as i32)
						.bind(*segment_id)
						.bind(normalize_float(*start_time))
						.bind(normalize_float(start_time + duration))
						.bind(size as i64)
						.build()
						.execute(global.db())
						.await
						.context("insert segment")? != 1
						{
							anyhow::bail!("no rows affected");
						}
					}
					RecordingTask::Init { data } => {
						bucket
							.put_object(
								video_common::keys::s3_init(organization_id, recording_id, rendition),
								data.clone(),
								Some(PutObjectOptions {
									content_type: Some("video/mp4".to_owned()),
									acl: Some(ObjectCannedAcl::PublicRead),
								}),
							)
							.await
							.context("upload init")?;
					}
				}

				Ok(())
			},
			5,
		)
		.await
		.context("s3_recording_task")?;
	}

	Ok(())
}

pub struct RecordingThumbnailTask {
	pub idx: u32,
	pub id: Ulid,
	pub start_time: f64,
	pub data: Bytes,
}

pub async fn recording_thumbnail_task(
	global: Arc<impl TranscoderGlobal>,
	organization_id: Ulid,
	recording_id: Ulid,
	bucket: binary_helper::s3::Bucket,
	mut rx: mpsc::Receiver<RecordingThumbnailTask>,
) -> anyhow::Result<()> {
	while let Some(task) = rx.recv().await {
		retry_task(
			|| async {
				let size = task.data.len();

				bucket
					.put_object(
						video_common::keys::s3_thumbnail(organization_id, recording_id, task.idx, task.id),
						task.data.clone(),
						Some(PutObjectOptions {
							content_type: Some("image/jpeg".to_owned()),
							acl: Some(ObjectCannedAcl::PublicRead),
						}),
					)
					.await
					.context("upload thumbnail")?;

				if scuffle_utils::database::query(
					r#"
                INSERT INTO recording_thumbnails (
                    organization_id,
                    recording_id,
                    idx,
                    id,
                    start_time,
                    size_bytes
                ) VALUES (
                    $1,
                    $2,
                    $3,
                    $4,
                    $5
                )"#,
				)
				.bind(organization_id)
				.bind(recording_id)
				.bind(task.idx as i32)
				.bind(task.id)
				.bind(normalize_float(task.start_time))
				.bind(size as i64)
				.build()
				.execute(global.db())
				.await
				.context("insert thumbnail")?
					!= 1
				{
					anyhow::bail!("no rows affected");
				}

				Ok(())
			},
			5,
		)
		.await
		.context("s3_thumbnail_task")?;
	}

	Ok(())
}
