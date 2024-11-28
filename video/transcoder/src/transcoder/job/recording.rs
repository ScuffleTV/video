use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use anyhow::{Context, Result};
use aws_config::Region;
use aws_sdk_s3::config::Credentials;
use bytes::Bytes;
use pb::ext::UlidExt;
use pb::scuffle::video::internal::live_rendition_manifest::recording_data::RecordingThumbnail;
use pb::scuffle::video::v1::types::{AudioConfig, RecordingConfig, Rendition as PbRendition, VideoConfig};
use prost::Message;
use scuffle_utils::database::tokio_postgres::Transaction;
use scuffle_utils::task::AsyncTask;
use tokio::sync::mpsc;
use ulid::Ulid;
use video_common::database::{Rendition, S3Bucket, Visibility};

use super::task::recording::{recording_task, recording_thumbnail_task, RecordingTask, RecordingThumbnailTask};
use crate::global::TranscoderGlobal;

pub struct PartialUpload {
	segment_id: Ulid,
	segment_idx: u32,
	duration: f64,
	start_time: f64,
	parts: Vec<Bytes>,
}

pub struct Recording {
	id: Ulid,
	allow_dvr: bool,
	partial_uploads: HashMap<Rendition, PartialUpload>,
	uploaders: HashMap<Rendition, mpsc::Sender<RecordingTask>>,
	thumbnail_uploader: mpsc::Sender<RecordingThumbnailTask>,
	tasks: Vec<AsyncTask<anyhow::Result<()>>>,
	renditions: HashSet<Rendition>,
	previous_thumbnails: Vec<RecordingThumbnail>,
}

impl Recording {
	#[allow(clippy::too_many_arguments)]
	pub async fn new(
		global: &Arc<impl TranscoderGlobal>,
		tx: &Transaction<'_>,
		id: Ulid,
		organization_id: Ulid,
		room_id: Ulid,
		visibility: Visibility,
		audio_outputs: &[AudioConfig],
		video_outputs: &[VideoConfig],
		s3_bucket: &S3Bucket,
		recording_config: &RecordingConfig,
	) -> Result<Self> {
		let bucket = binary_helper::s3::Bucket::new(
			s3_bucket.name.clone(),
			Credentials::from_keys(&s3_bucket.access_key_id, &s3_bucket.secret_access_key, None),
			Region::new(s3_bucket.region.clone()),
			s3_bucket.endpoint.clone(),
		);

		let recording_renditions = audio_outputs
			.iter()
			.map(|o| (o.rendition, o.encode_to_vec()))
			.chain(video_outputs.iter().map(|o| (o.rendition, o.encode_to_vec())))
			.filter(|(r, _)| recording_config.renditions.contains(r))
			.map(|(r, config)| (Rendition::from(PbRendition::try_from(r).unwrap_or_default()), config))
			.collect::<Vec<_>>();

		let allow_dvr = recording_renditions.len() == video_outputs.len() + audio_outputs.len();

		scuffle_utils::database::query(
			r#"
			INSERT INTO recordings (
                id,
                organization_id,
                room_id,
                recording_config_id,
                visibility,
                allow_dvr,
                s3_bucket_id
            ) VALUES (
                $1,
                $2,
                $3,
                $4,
                $5,
                $6,
                $7
            ) ON CONFLICT DO NOTHING
			"#,
		)
		.bind(id)
		.bind(organization_id)
		.bind(room_id)
		.bind(recording_config.id.into_ulid())
		.bind(visibility)
		.bind(allow_dvr)
		.bind(s3_bucket.id)
		.build()
		.execute(tx)
		.await?;

		scuffle_utils::database::query(
			"INSERT INTO recording_renditions (organization_id, recording_id, rendition, config)",
		)
		.push_values(recording_renditions.iter(), |mut b, (rendition, config)| {
			b.push_bind(organization_id);
			b.push_bind(id);
			b.push_bind(rendition);
			b.push_bind(config);
		})
		.push("ON CONFLICT DO NOTHING")
		.build()
		.execute(tx)
		.await?;

		let mut tasks = Vec::new();
		let mut uploaders = HashMap::new();

		for (rendition, _) in &recording_renditions {
			let (tx, rx) = mpsc::channel(16);
			uploaders.insert(*rendition, tx);
			tasks.push(AsyncTask::new(
				format!("recording({rendition})"),
				recording_task(global.clone(), organization_id, id, *rendition, bucket.clone(), rx),
			));
		}

		let (tx, rx) = mpsc::channel(16);
		tasks.push(AsyncTask::new(
			"recording(thumbnail)",
			recording_thumbnail_task(global.clone(), organization_id, room_id, bucket.clone(), rx),
		));

		Ok(Self {
			id,
			allow_dvr,
			renditions: recording_renditions.into_iter().map(|(r, _)| r).collect(),
			partial_uploads: HashMap::new(),
			uploaders,
			tasks,
			previous_thumbnails: Vec::new(),
			thumbnail_uploader: tx,
		})
	}

	pub fn recover_thumbnails(&mut self, thumbnails: Vec<RecordingThumbnail>) {
		self.previous_thumbnails = thumbnails;
	}

	pub fn thumbnails(&self) -> &[RecordingThumbnail] {
		&self.previous_thumbnails
	}

	pub fn id(&self) -> Ulid {
		self.id
	}

	pub fn allow_dvr(&self) -> bool {
		self.allow_dvr
	}

	pub fn tasks(&mut self) -> Vec<AsyncTask<anyhow::Result<()>>> {
		std::mem::take(&mut self.tasks)
	}

	#[allow(clippy::too_many_arguments)]
	pub fn upload_part(
		&mut self,
		rendition: Rendition,
		id: Ulid,
		idx: u32,
		data: Bytes,
		start_time: f64,
		duration: f64,
		finished: bool,
	) -> anyhow::Result<()> {
		if !self.renditions.contains(&rendition) {
			return Ok(());
		}

		let partial_upload = self.partial_uploads.entry(rendition).or_insert_with(|| PartialUpload {
			segment_id: id,
			segment_idx: idx,
			duration,
			start_time,
			parts: Vec::new(),
		});

		if partial_upload.segment_id != id || finished {
			let partial_upload = std::mem::replace(
				partial_upload,
				PartialUpload {
					segment_id: id,
					segment_idx: idx,
					duration,
					start_time,
					parts: vec![data],
				},
			);

			self.uploaders
				.get_mut(&rendition)
				.unwrap()
				.try_send(RecordingTask::Segment {
					segment_id: partial_upload.segment_id,
					segment_idx: partial_upload.segment_idx,
					duration: partial_upload.duration,
					start_time: partial_upload.start_time,
					parts: partial_upload.parts,
				})
				.context("send upload task")?;
		} else {
			partial_upload.parts.push(data);
			partial_upload.duration = duration;
		}

		Ok(())
	}

	pub fn upload_init(&mut self, rendition: Rendition, data: Bytes) -> anyhow::Result<()> {
		if !self.renditions.contains(&rendition) {
			return Ok(());
		}

		self.uploaders
			.get_mut(&rendition)
			.unwrap()
			.try_send(RecordingTask::Init { data })
			.context("send init task")?;

		Ok(())
	}

	pub fn upload_thumbnail(&mut self, idx: u32, start_time: f64, data: Bytes) -> anyhow::Result<()> {
		if self.previous_thumbnails.len() >= 10 {
			self.previous_thumbnails.remove(0);
		}

		let id = Ulid::new();

		self.previous_thumbnails.push(RecordingThumbnail {
			ulid: Some(id.into()),
			idx,
			timestamp: ((start_time * 1000.0) as f32).round() / 1000.0,
		});

		self.thumbnail_uploader
			.try_send(RecordingThumbnailTask {
				id,
				idx,
				start_time,
				data,
			})
			.context("send thumbnail task")?;

		Ok(())
	}
}
