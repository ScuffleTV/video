use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Context;
use bytes::Bytes;
use pb::scuffle::video::internal::live_rendition_manifest::{self, RecordingData, RenditionInfo, Segment};
use pb::scuffle::video::internal::LiveRenditionManifest;
use prost::Message;
use tokio::sync::mpsc;
use video_common::database::Rendition;

use super::recording::Recording;
use super::task::rendition::TrackTask;
use crate::global::TranscoderGlobal;

pub mod parser;
pub mod state;

pub struct Track {
	rendition: Rendition,
	state: state::TrackState,
	uploader: mpsc::Sender<TrackTask>,
	target_part_duration: f64,
	max_part_duration: f64,
	min_segment_duration: f64,
	ready: bool,
	previous_segments: Vec<Segment>,
}

impl Track {
	pub fn new(global: &Arc<impl TranscoderGlobal>, rendition: Rendition, uploader: mpsc::Sender<TrackTask>) -> Self {
		Self {
			rendition,
			state: state::TrackState::default(),
			uploader,
			target_part_duration: global.config().target_part_duration.as_secs_f64(),
			max_part_duration: global.config().max_part_duration.as_secs_f64(),
			min_segment_duration: global.config().min_segment_duration.as_secs_f64(),
			ready: false,
			previous_segments: Vec::new(),
		}
	}

	pub fn ready(&mut self, recording: Option<&mut Recording>) -> anyhow::Result<()> {
		assert!(self.init_segment().is_some(), "ready called before init segment");
		self.ready = true;
		self.upload_init()?;
		self.handle_samples(recording)?;
		Ok(())
	}

	pub fn upload_init(&mut self) -> anyhow::Result<()> {
		if !self.ready {
			return Ok(());
		}

		self.uploader
			.try_send(TrackTask::Init {
				data: self.state.init_segment().unwrap().clone(),
			})
			.context("send init task")?;
		Ok(())
	}

	pub fn info(&self) -> RenditionInfo {
		RenditionInfo {
			next_part_idx: self.state.next_part_idx(),
			next_segment_idx: self.state.next_segment_idx(),
			next_segment_part_idx: self.state.next_segment_part_idx(),
			last_independent_part_idx: self.state.last_independent_part_idx(),
		}
	}

	pub fn handle_track_out(
		&mut self,
		recording: Option<&mut Recording>,
		track_out: parser::TrackOut,
	) -> anyhow::Result<bool> {
		match track_out {
			parser::TrackOut::Moov(moov) => {
				self.state.set_moov(moov);
				Ok(false)
			}
			parser::TrackOut::Samples(samples) => {
				self.state.append_samples(samples);
				self.handle_samples(recording)
			}
		}
	}

	pub fn finish(&mut self, mut recording: Option<&mut Recording>) -> anyhow::Result<()> {
		if !self.ready {
			return Ok(());
		}

		if let Some((segment_idx, part_idx)) = self.state.finish() {
			self.handle_addtion(&mut recording, segment_idx, part_idx)?;
		}

		Ok(())
	}

	pub fn init_segment(&self) -> Option<&Bytes> {
		self.state.init_segment()
	}

	fn handle_samples(&mut self, mut recording: Option<&mut Recording>) -> anyhow::Result<bool> {
		if !self.ready {
			return Ok(false);
		}

		let additions =
			self.state
				.split_samples(self.target_part_duration, self.max_part_duration, self.min_segment_duration);

		let mut has_additions = false;

		for (segment_idx, parts) in additions {
			for part_idx in parts {
				self.handle_addtion(&mut recording, segment_idx, part_idx)?;
				has_additions = true;
			}
		}

		self.state.retain_segments(5);

		Ok(has_additions)
	}

	fn handle_addtion(
		&mut self,
		recording: &mut Option<&mut Recording>,
		segment_idx: u32,
		part_idx: u32,
	) -> anyhow::Result<f64> {
		let segment = self.state.segment(segment_idx).unwrap();

		let part = segment.part(part_idx).unwrap();

		if let Some(recording) = recording {
			recording
				.upload_part(
					self.rendition,
					segment.id,
					segment.idx,
					part.data.clone(),
					segment.parts.first().map(|p| p.start_ts).unwrap_or_default() as f64 / self.state.timescale() as f64,
					segment.duration() as f64 / self.state.timescale() as f64,
					false,
				)
				.context("recording")?;
		}

		self.uploader
			.try_send(TrackTask::Media {
				part_idx,
				data: part.data.clone(),
			})
			.context("send media task")?;

		Ok(part.duration as f64 / self.state.timescale() as f64)
	}

	pub fn apply_manifest(&mut self, manifest: LiveRenditionManifest) {
		self.state.apply_manifest(&manifest);
		self.previous_segments = manifest.segments;
	}

	pub fn update_manifest(
		&mut self,
		recording: Option<&mut Recording>,
		info_map: &HashMap<String, RenditionInfo>,
		shutdown: bool,
	) -> anyhow::Result<()> {
		if !self.ready {
			return Ok(());
		}

		let completed = self.state.complete() && shutdown;

		let mut manifest = LiveRenditionManifest {
			info: None,
			other_info: HashMap::new(),
			completed,
			timescale: self.state.timescale(),
			total_duration: self.state.total_duration(),
			recording_data: if let Some(recording) = &recording {
				if recording.allow_dvr() {
					Some(RecordingData {
						recording_ulid: Some(recording.id().into()),
						thumbnails: recording.thumbnails().to_vec(),
					})
				} else {
					None
				}
			} else {
				None
			},
			segments: self
				.state
				.segments()
				.map(|s| live_rendition_manifest::Segment {
					idx: s.idx,
					id: Some(s.id.into()),
					parts: s
						.parts
						.iter()
						.map(|p| live_rendition_manifest::Part {
							idx: p.idx,
							duration: p.duration,
							independent: p.independent,
						})
						.collect(),
				})
				.collect(),
		};

		if !completed && self.previous_segments == manifest.segments {
			return Ok(());
		}

		let mut info_map = info_map.clone();
		manifest.info = info_map.remove(&self.rendition.to_string());
		manifest.other_info = info_map;

		let data = Bytes::from(manifest.encode_to_vec());

		self.uploader
			.try_send(TrackTask::Manifest { data })
			.context("send manifest task")?;

		self.previous_segments = manifest.segments;

		Ok(())
	}
}
