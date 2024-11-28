use std::cmp::Ordering;

use crate::player::events;
use crate::player::runner::blank::VideoFactory;
use crate::player::runner::Runner;

impl Runner {
	pub fn set_realtime_mode(&mut self, realtime: bool) {
		let set = self.inner.borrow_mut().set_realtime(realtime);
		if let Ok(true) = set {
			events::dispatch!(self.inner.borrow_mut().events.emit(events::UserEvent::Realtime));
		}
	}

	pub fn buffer_end(&self) -> Option<f64> {
		let buffered = self.element.buffered();
		if buffered.length() == 0 {
			tracing::trace!("buffered length is 0");
			return None;
		}

		let current_time = self.element.current_time();
		for i in (0..buffered.length()).rev() {
			let start = buffered.start(i).unwrap_or(0.0);
			let end = buffered.end(i).unwrap_or(0.0);
			if current_time >= start && current_time <= end {
				return Some(end);
			}
		}

		tracing::trace!("no buffer end found");

		None
	}

	pub fn abr_variant_id(&self) -> Option<u32> {
		let bandwidth = self.inner.borrow().bandwidth.estimate();

		let variants = self
			.inner
			.borrow()
			.runner_settings
			.variants
			.iter()
			.enumerate()
			.map(|(i, v)| {
				(
					i,
					v.audio_track.bitrate + v.video_track.as_ref().map(|t| t.bitrate).unwrap_or_default(),
					v.video_track.is_some(),
				)
			})
			.collect::<Vec<_>>();

		let active_vid = self.inner.borrow().runner_settings.current_variant_id as usize;

		// We have some bandwidth estimation, so we should try do some ABR to get the
		// best quality
		let (id, _, _) = variants
			.iter()
			.find(|(id, vb, has_video)| {
				if !has_video {
					return false;
				}

				if id == &active_vid {
					*vb as f64 <= bandwidth
				} else {
					let mut vbf = *vb as f64;
					if let Some(buffer_size) = self.buffer_end().map(|b| self.element.current_time() - b) {
						vbf += variants[active_vid].1 as f64 * (0.75 - buffer_size).max(0.0);
					}

					// The problem with switching to a higher track is we need enough bandwidth to
					// load our current track and then load the next track.
					match variants[active_vid].1.cmp(vb) {
						// Encourage switching to a lower bitrate.
						Ordering::Less => vbf <= bandwidth,
						// Discourage switching to a higher bandwidth unless we have 0.5mbps spare.
						_ => vbf + 0.5 * 1000.0 * 1000.0 <= bandwidth,
					}
				}
			})
			.or_else(|| variants.iter().rev().find(|(_, _, has_video)| *has_video))?;

		let id = *id as u32;
		if id == active_vid as u32 { None } else { Some(id) }
	}

	pub async fn init_audio(&mut self) {
		if self.audio_init != Some(self.active_audio_track_idx) {
			tracing::trace!(
				"init audio is set to {:?} but we want {}",
				self.audio_init,
				self.active_audio_track_idx
			);

			let audio_track = &self.audio_tracks[self.active_audio_track_idx];
			let Some(init) = audio_track.init_segment() else {
				tracing::trace!(
					"no init segment for audio track {}, skipping init",
					self.active_audio_track_idx
				);
				return;
			};

			self.audio_init = Some(self.active_audio_track_idx);

			let codec = format!("audio/mp4; codecs=\"{}\"", audio_track.track().codec);

			tracing::debug!("changing audio type to {}", codec);

			self.source_buffers.as_mut().unwrap().audio.change_type(&codec).await.unwrap();
			self.source_buffers
				.as_mut()
				.unwrap()
				.audio
				.append_buffer(init.clone().into())
				.await
				.unwrap();

			if self.active_video_track_idx.is_none() {
				tracing::debug!(
					"no video track, creating blank video track factory, with timescale {}",
					audio_track.timescle()
				);
				self.video_init = None;
				self.video_factory = Some(VideoFactory::new(audio_track.timescle()));
				self.init_video().await;
			}
		}
	}

	pub async fn init_video(&mut self) {
		if let Some(video_factory) = self.video_factory.as_mut() {
			if self.video_init != Some(None) {
				tracing::trace!("video init is set to {:?} but we want None", self.video_init);

				self.video_init = Some(None);

				let codec = format!("video/mp4; codecs=\"{}\"", video_factory.codec());

				tracing::debug!("changing video type to {}", codec);

				self.source_buffers.as_mut().unwrap().video.change_type(&codec).await.unwrap();

				self.source_buffers
					.as_mut()
					.unwrap()
					.video
					.append_buffer(video_factory.init_segment().into())
					.await
					.unwrap();
			}
		} else if self.active_video_track_idx.is_some() && self.video_init != Some(self.active_video_track_idx) {
			tracing::trace!(
				"video init is set to {:?} but we want {}",
				self.video_init,
				self.active_video_track_idx.unwrap()
			);

			let video_track = &self.video_tracks[self.active_video_track_idx.unwrap()];
			let Some(init) = video_track.init_segment() else {
				tracing::trace!(
					"no init segment for video track {}, skipping init",
					self.active_video_track_idx.unwrap()
				);
				return;
			};

			self.video_init = Some(self.active_video_track_idx);

			let codec = format!("video/mp4; codecs=\"{}\"", video_track.track().codec);
			tracing::debug!("changing video type to {}", codec);

			self.source_buffers.as_mut().unwrap().video.change_type(&codec).await.unwrap();
			self.source_buffers
				.as_mut()
				.unwrap()
				.video
				.append_buffer(init.clone().into())
				.await
				.unwrap();
		}
	}
}
