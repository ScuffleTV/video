use std::ops::{Add, Sub};

use wasm_bindgen::JsCast;

use crate::player::errors::{ErrorCode, EventError, EventErrorExt, EventErrorExtFetch};
use crate::player::inner::{NextVariant, NextVariantAutoCause};
use crate::player::runner::blank::VideoFactory;
use crate::player::runner::utils::VideoElementEvent;
use crate::player::runner::Runner;
use crate::player::{events, PlayerResult};

impl Runner {
	pub fn handle_video_events(&mut self, now: f64) {
		while let Ok(evt) = self.element.events().try_recv() {
			match evt {
				VideoElementEvent::Seeking => {
					self.set_realtime_mode(
						self.element.seekable().end(0).unwrap_or(0.0) - self.element.current_time()
							< if self.inner.borrow().interface_settings.player_settings.enable_low_latency {
								self.inner
									.borrow()
									.interface_settings
									.player_settings
									.low_latency_realtime_threshold_ms
							} else {
								self.inner
									.borrow()
									.interface_settings
									.player_settings
									.normal_latency_realtime_threshold_ms
							} / 1000.0,
					);
					tracing::debug!("seeking, {}", self.inner.borrow().interface_settings.realtime_mode);
					self.timings.last_seeked = now;

					if let Some(next_audio_track_idx) = self.next_audio_track_idx {
						self.audio_tracks[self.active_audio_track_idx].stop();
						self.audio_tracks[next_audio_track_idx].start();
						self.active_audio_track_idx = next_audio_track_idx;
					}

					if let Some(next_video_track_idx) = self.next_video_track_idx {
						if let Some(old_video_track_idx) = self.active_video_track_idx.take() {
							self.video_tracks[old_video_track_idx].stop();
						}
						self.video_tracks[next_video_track_idx].start();
						self.active_video_track_idx = Some(next_video_track_idx);
					}
				}
				VideoElementEvent::TimeUpdate => {
					self.timings.waiting = None;
					self.timings.last_time_update = now;
					self.timings.current_player_time = self.element.current_time();
				}
				VideoElementEvent::Pause => {
					self.set_realtime_mode(false);
					self.inner.borrow_mut().interface_settings.auto_seek = false;
				}
				VideoElementEvent::Play => {
					self.set_realtime_mode(
						(self.element.seekable().end(0).unwrap_or(0.0) - self.element.current_time()).abs()
							< if self.inner.borrow().interface_settings.player_settings.enable_low_latency {
								self.inner
									.borrow()
									.interface_settings
									.player_settings
									.low_latency_realtime_threshold_ms
							} else {
								self.inner
									.borrow()
									.interface_settings
									.player_settings
									.normal_latency_realtime_threshold_ms
							} / 1000.0,
					);

					self.timings.last_time_update = now;
				}
				VideoElementEvent::Playing => {
					self.timings.last_time_update = now;
				}
				VideoElementEvent::Waiting => {
					self.timings.waiting = Some(now);
					// When we are buffering we want to be able to switch to a lower quality sooner.
					self.timings.last_abr_switch -= 3.0;
				}
				VideoElementEvent::Error(err) => {
					events::dispatch!(
						self.inner.borrow_mut().events.emit(events::UserEvent::Error(
							EventError::new(ErrorCode::Decode, "video element error".into(), true,)
								.with_source(err.unchecked_into())
						))
					);
				}
			}
		}
	}

	pub fn handle_rate_control(&mut self, now: f64) {
		if !self.inner.borrow().interface_settings.realtime_mode {
			if self.playback_factor != self.inner.borrow().interface_settings.player_settings.normal_playback_rate {
				self.element
					.set_playback_rate(self.inner.borrow().interface_settings.player_settings.normal_playback_rate);
				self.playback_factor = self.inner.borrow().interface_settings.player_settings.normal_playback_rate;
			}

			return;
		}

		if !self.element.paused()
			&& now - self.timings.last_rate_change
				> self
					.inner
					.borrow()
					.interface_settings
					.player_settings
					.playback_rate_change_cooldown_ms
		{
			let buffer_target = if self.inner.borrow().interface_settings.player_settings.enable_low_latency {
				self.inner
					.borrow()
					.interface_settings
					.player_settings
					.low_latency_target_buffer_duration_ms
			} else {
				self.inner
					.borrow()
					.interface_settings
					.player_settings
					.normal_latency_target_buffer_duration_ms
			} / 1000.0;

			if self.playback_factor != self.element.playback_rate() {
				// The user changed the playback rate, so we don't want to change it.
				self.timings.last_rate_change = now;
				self.playback_factor = self.inner.borrow().interface_settings.player_settings.normal_playback_rate;
				tracing::debug!("user changed playback rate, ignoring");
				return;
			}

			let Some(buffer_end) = self.buffer_end() else {
				return;
			};

			let buffer_size = buffer_end - self.timings.current_player_time;

			let new_rate = if buffer_size > buffer_target + 0.5 {
				self.inner.borrow().interface_settings.player_settings.fast_playback_rate
			} else if buffer_size < buffer_target - 0.5 {
				self.inner.borrow().interface_settings.player_settings.slow_playback_rate
			} else {
				self.inner.borrow().interface_settings.player_settings.normal_playback_rate
			};

			if self.playback_factor != new_rate {
				self.element.set_playback_rate(new_rate);
				self.timings.last_rate_change = now;
				self.playback_factor = new_rate;
			}
		}
	}

	pub fn handle_next_variant(&mut self) {
		let next_id = self.inner.borrow().interface_settings.next_variant_id;
		if let Some(next_id) = next_id {
			tracing::trace!("handling next variant: {:?}", next_id);

			let next_vid = next_id.variant_id();

			let (audio_id, video_id) = self
				.inner
				.borrow()
				.runner_settings
				.variants
				.get(next_vid as usize)
				.map(|v| (v.audio_track.id, v.video_track.as_ref().map(|v| v.id)))
				.unwrap();

			if self.active_audio_track_idx != audio_id {
				tracing::trace!("audio track changed: {} -> {audio_id}", self.active_audio_track_idx);

				if next_id.is_force() {
					// If the user forced the track switch, we don't care about the current
					// track's state.
					if let Some(audio_id) = self.next_audio_track_idx.take() {
						tracing::trace!("aborting track switch: {audio_id}, clearing temporary audio buffer");
						self.temporary_audio_buffer.clear();
						self.audio_tracks[audio_id].stop();
					}

					tracing::trace!("stopping current audio track: {}", self.active_audio_track_idx);
					self.audio_tracks[self.active_audio_track_idx].stop();

					if self.inner.borrow().interface_settings.realtime_mode
						&& self.inner.borrow().interface_settings.player_settings.enable_low_latency
					{
						let idx = self.audio_tracks[self.active_audio_track_idx]
							.rendition_info(self.audio_tracks[audio_id].track().name.as_str())
							.map(|r| r.last_independent_part_idx)
							.unwrap_or_default();
						tracing::trace!("starting audio track {audio_id} at last independent part: {}", idx);
						self.audio_tracks[audio_id].start_at_part(idx)
					} else {
						tracing::trace!(
							"starting audio track {audio_id} from current player time: {}",
							self.timings.current_player_time
						);
						self.audio_tracks[audio_id].start_from_time(self.timings.current_player_time)
					}

					tracing::debug!("setting active audio track: {audio_id}");
					self.active_audio_track_idx = audio_id;
				} else if self.next_audio_track_idx != Some(audio_id) {
					// This is to check if there was currently a track switch in progress.
					if let Some(audio_id) = self.next_audio_track_idx {
						// If so we abort the switch.
						tracing::trace!("aborting track switch: {audio_id}, clearing temporary audio buffer");
						self.temporary_audio_buffer.clear();
						self.audio_tracks[audio_id].stop();
					}

					if self.inner.borrow().interface_settings.realtime_mode
						&& self.inner.borrow().interface_settings.player_settings.enable_low_latency
					{
						let idx = self.audio_tracks[self.active_audio_track_idx]
							.rendition_info(self.audio_tracks[audio_id].track().name.as_str())
							.map(|r| r.last_independent_part_idx)
							.unwrap_or_default();
						tracing::trace!("starting audio track {audio_id} at independent part after: {}", idx);
						self.audio_tracks[audio_id].start_at_ipart(idx);
					} else {
						let time = if self.inner.borrow().interface_settings.realtime_mode {
							self.buffer_end().unwrap_or(self.timings.current_player_time + 1.0)
						} else {
							self.audio_tracks[self.active_audio_track_idx]
								.range_duration()
								.map(|r| r.end)
								.unwrap_or(self.timings.current_player_time)
						};

						tracing::trace!("starting audio track {audio_id} from time: {time}");
						self.audio_tracks[audio_id].start_from_time(time);
					}

					self.next_audio_track_idx = Some(audio_id);
				}
			} else if let Some(audio_id) = self.next_audio_track_idx.take() {
				// If we were busy switching to a new track, but then switched back to the
				// active track, we abort the switch.
				tracing::trace!("aborting track switch: {audio_id}, clearing temporary audio buffer");
				self.temporary_audio_buffer.clear();
				self.video_tracks[audio_id].stop();
			}

			if let Some(video_id) = video_id {
				if self.active_video_track_idx != Some(video_id) {
					tracing::trace!("video track changed: {:?} -> {video_id}", self.active_video_track_idx);
					if next_id.is_force() {
						// If the user forced the track switch, we don't care about the current
						// track's state.
						if let Some(video_id) = self.next_video_track_idx.take() {
							tracing::trace!("aborting track switch: {video_id}, clearing temporary video buffer");
							self.temporary_video_buffer.clear();
							self.video_tracks[video_id].stop();
						}

						// Since we are force changing the track, we can just stop the current
						// track immediately.
						if let Some(video_id) = self.active_video_track_idx.take() {
							tracing::trace!("stopping current video track: {video_id}");
							self.video_tracks[video_id].stop();
						} else {
							tracing::trace!("stopping current video track factory");
							self.video_factory = None;
						}

						// If we are in realtime mode and low latency mode, we want to start the
						// track at the last independent part.
						if self.inner.borrow().interface_settings.realtime_mode
							&& self.inner.borrow().interface_settings.player_settings.enable_low_latency
						{
							let idx = self.audio_tracks[self.active_audio_track_idx]
								.rendition_info(self.video_tracks[video_id].track().name.as_str())
								.map(|r| r.last_independent_part_idx)
								.unwrap_or_default();
							tracing::trace!("starting video track {video_id} at last independent part: {idx}");
							self.video_tracks[video_id].start_at_part(idx)
						} else {
							// Otherwise we want to start the track from the current player time.
							tracing::trace!(
								"starting video track {video_id} from current player time: {}",
								self.timings.current_player_time
							);
							self.video_tracks[video_id].start_from_time(self.timings.current_player_time)
						}

						// We can then just set the active track to the new track.
						tracing::debug!("setting active video track: {video_id}");
						self.active_video_track_idx = Some(video_id);
					} else if self.next_video_track_idx != Some(video_id) {
						// This is to check if there was currently a track switch in progress.
						if let Some(video_id) = self.next_video_track_idx {
							tracing::trace!("aborting track switch: {video_id}, clearing temporary video buffer");
							self.temporary_video_buffer.clear();
							self.video_tracks[video_id].stop();
						}

						// If we are in low latency mode, we want to start the track at the next
						// independent part.
						if self.inner.borrow().interface_settings.realtime_mode
							&& self.inner.borrow().interface_settings.player_settings.enable_low_latency
						{
							let idx = self.audio_tracks[self.active_audio_track_idx]
								.rendition_info(self.video_tracks[video_id].track().name.as_str())
								.map(|r| r.last_independent_part_idx)
								.unwrap_or_default();
							tracing::trace!("starting video track {video_id} at independent part after: {idx}");
							self.video_tracks[video_id].start_at_ipart(idx);
						} else {
							// Otherwise we want to start the track from when the buffer ends.
							let time = if self.inner.borrow().interface_settings.realtime_mode {
								self.buffer_end().unwrap_or(self.timings.current_player_time + 1.0)
							} else {
								self.active_video_track_idx
									.and_then(|idx| self.video_tracks[idx].range_duration().map(|r| r.end))
									.unwrap_or(self.timings.current_player_time)
							};
							tracing::trace!("starting video track {video_id} from time: {time}");
							self.video_tracks[video_id].start_from_time(time);
						}

						tracing::debug!("setting active video track: {video_id}");
						self.next_video_track_idx = Some(video_id);
					}
				} else if let Some(video_id) = self.next_video_track_idx.take() {
					// If we were busy switching to a new track, but then switched back to the
					// active track, we abort the switch.
					tracing::trace!("aborting track switch: {video_id}, clearing temporary video buffer");
					self.temporary_video_buffer.clear();
					self.video_tracks[video_id].stop();
				}
			} else if let Some(video_id) = self.active_video_track_idx.take() {
				// If we are switching to an audio only variant, we dont need to keep the video
				// track. We can just stop it immediately.
				tracing::trace!("switching to audio only variant, stopping video track: {video_id}, creating video factory");
				self.video_factory = Some(VideoFactory::new(self.audio_tracks[self.active_audio_track_idx].timescle()));
				self.video_tracks[video_id].stop();
			}

			// Check if the current active tracks are what is expected.
			if self.active_audio_track_idx == audio_id && self.active_video_track_idx == video_id {
				tracing::debug!("switched to variant: {next_vid}, audio: {audio_id}, video: {:?}", video_id);
				let previous_variant_id = self.inner.borrow().runner_settings.current_variant_id;

				self.inner.borrow_mut().runner_settings.current_variant_id = next_vid;
				self.inner.borrow_mut().interface_settings.next_variant_id = None;

				if previous_variant_id != next_vid {
					events::dispatch!(self.inner.borrow_mut().events.emit(events::UserEvent::Variant(
						events::VariantEvent {
							variant_id: next_vid,
							automatic: next_id.automatic(),
							previous_variant_id: previous_variant_id as i32,
						}
					)));
				} else {
					tracing::debug!("variant change was a no-op");
				}
			} else {
				tracing::trace!(
					"waiting for track switch: audio: {} -> {audio_id}, video: {:?} -> {:?}",
					self.active_audio_track_idx,
					self.active_video_track_idx,
					video_id,
				);
			}
		}
	}

	pub async fn handle_buffer_size(&mut self) -> PlayerResult<()> {
		let Some(source_buffers) = &mut self.source_buffers else {
			tracing::trace!("no source buffers, skipping buffer size check");
			return Ok(());
		};

		for sb in [&mut source_buffers.audio, &mut source_buffers.video] {
			let Ok(buffered) = sb.buffered() else {
				tracing::trace!("no buffered regions, skipping buffer size check");
				continue;
			};

			let last_buffered_idx = buffered.length().saturating_sub(1);

			let current_time = self.element.current_time();

			for i in 0..buffered.length() {
				let start = buffered.start(i).unwrap_or(0.0);
				let mut end = buffered.end(i).unwrap_or(0.0);

				let min_start = start.sub(10.0).min(0.0);
				let min_end = end.add(10.0);

				if current_time >= min_start && (current_time <= min_end || last_buffered_idx == i) {
					let dist = current_time - start;
					if dist > 30.0 && end - start > 30.0 {
						end = start + 15.0;
					} else {
						continue;
					}
				}

				tracing::debug!("removing buffered region: {start} - {end}");

				sb.remove(start, end)
					.await
					.other_error("failed to remove buffered range", true)?;

				self.video_tracks.iter_mut().for_each(|t| t.remove_regions(start, end));
				self.video_tracks.iter_mut().for_each(|t| t.remove_regions(start, end));
			}
		}

		Ok(())
	}

	pub fn handle_buffer_hole(&mut self, now: f64) {
		if self.element.paused() {
			return;
		}

		if self.inner.borrow().interface_settings.auto_seek {
			self.handle_autoseek();
			return;
		}

		// We have stalled
		if now - self.timings.last_time_update > 500.0 && !self.flags.is_finished {
			if !self.inner.borrow().interface_settings.realtime_mode {
				self.element.pause().ok();
			} else {
				self.timings.last_time_update = now;
				self.inner.borrow_mut().interface_settings.auto_seek = true;
				if !self.element.seeking() {
					tracing::warn!("buffer hole detected, seeking to end of buffer");
				} else {
					tracing::debug!("buffer hole detected, already seeking");
				}
				self.handle_autoseek();
			}
		}
	}

	pub fn handle_autoseek(&mut self) {
		if !self.inner.borrow_mut().interface_settings.auto_seek {
			return;
		}

		self.inner.borrow_mut().interface_settings.auto_seek = false;

		if !self.inner.borrow().interface_settings.realtime_mode {
			tracing::debug!("seeking to 0.0");
			self.element.set_current_time(0.0);
		}
		if self.element.seekable().length() > 0 {
			let time = (self.element.seekable().end(0).unwrap()
				- if self.inner.borrow().interface_settings.player_settings.enable_low_latency {
					self.inner
						.borrow()
						.interface_settings
						.player_settings
						.low_latency_target_buffer_duration_ms
						/ 1000.0
				} else {
					self.inner
						.borrow()
						.interface_settings
						.player_settings
						.normal_latency_target_buffer_duration_ms
						/ 1000.0
				})
			.max(0.0);
			tracing::debug!("seeking to {time}");
			self.element.set_current_time(time);
		} else {
			tracing::debug!("not seeking because no seekable ranges");
			self.inner.borrow_mut().interface_settings.auto_seek = true;
		}
	}

	pub fn handle_bandwidth(&mut self, now: f64) {
		let (fast, slow) = if self.inner.borrow().interface_settings.realtime_mode {
			(
				self.inner
					.borrow()
					.interface_settings
					.player_settings
					.abr_fast_realtime_half_life,
				self.inner
					.borrow()
					.interface_settings
					.player_settings
					.abr_slow_realtime_half_life,
			)
		} else {
			(
				self.inner.borrow().interface_settings.player_settings.abr_fast_half_life,
				self.inner.borrow().interface_settings.player_settings.abr_slow_half_life,
			)
		};

		self.inner.borrow_mut().bandwidth.update_alpha(fast, slow);

		if now - self.timings.last_abr_switch > self.inner.borrow().interface_settings.player_settings.abr_switch_cooldown_ms
			&& self.inner.borrow().interface_settings.player_settings.enable_abr
			&& self.inner.borrow().interface_settings.next_variant_id.is_none()
			&& self.audio_only_auto_switch.is_none()
		{
			if let Some(abr_id) = self.abr_variant_id() {
				self.timings.last_abr_switch = now;
				tracing::debug!(
					"switching to {abr_id} because bandwidth is {}",
					self.inner.borrow().bandwidth.estimate()
				);

				self.inner.borrow_mut().interface_settings.next_variant_id = Some(NextVariant::Auto {
					id: abr_id,
					cause: NextVariantAutoCause::Bandwidth,
				});
			}
		}
	}

	pub fn handle_visibility(&mut self, now: f64) {
		if self.visibility_detector.visible() != self.inner.borrow().runner_settings.visible {
			self.inner.borrow_mut().runner_settings.visible = self.visibility_detector.visible();

			events::dispatch!(self.inner.borrow_mut().events.emit(events::UserEvent::Visibility,));
		}

		if self.visibility_detector.visible() {
			if self.timings.document_visible.is_some() {
				if let Some(old_variant_id) = self.audio_only_auto_switch.take() {
					let next_variant = if self.inner.borrow().interface_settings.player_settings.enable_abr {
						self.abr_variant_id().unwrap_or(old_variant_id)
					} else {
						old_variant_id
					};

					self.timings.last_abr_switch = now;

					self.inner.borrow_mut().interface_settings.next_variant_id = Some(NextVariant::Auto {
						id: next_variant,
						cause: NextVariantAutoCause::Visibility,
					});
				}

				self.timings.document_visible = None;
			}
		} else if self.timings.document_visible.is_none() {
			self.timings.document_visible = Some(now);
		}

		let Some(document_visible) = self.timings.document_visible else {
			return;
		};

		if now - document_visible
			< self
				.inner
				.borrow()
				.interface_settings
				.player_settings
				.audio_only_switch_delay_ms
		{
			return;
		}

		if !(self.inner.borrow().interface_settings.player_settings.auto_audio_only
			&& self.inner.borrow().interface_settings.next_variant_id.is_none()
			&& self.inner.borrow().interface_settings.realtime_mode
			&& self.active_video_track_idx.is_some())
		{
			return;
		}

		let Some(audio_only_variant_id) = self
			.inner
			.borrow()
			.runner_settings
			.variants
			.iter()
			.position(|v| v.video_track.is_none())
		else {
			return;
		};

		self.audio_only_auto_switch = Some(self.inner.borrow().runner_settings.current_variant_id);

		self.inner.borrow_mut().interface_settings.next_variant_id = Some(NextVariant::Auto {
			id: audio_only_variant_id as u32,
			cause: NextVariantAutoCause::Visibility,
		});
	}

	pub fn handle_session_refresh(&mut self, now: f64) -> PlayerResult<()> {
		if self.refresh_req.is_some() {
			return Ok(());
		}

		let session_refresh = self.timings.last_session_refresh.max(
			self.video_tracks
				.iter()
				.map(|t| t.last_session_refreshed())
				.chain(self.audio_tracks.iter().map(|t| t.last_session_refreshed()))
				.max_by(|a, b| a.partial_cmp(b).unwrap())
				.unwrap_or(-1.0),
		);

		if session_refresh == -1.0 {
			return Ok(());
		}

		if now - session_refresh
			> self
				.inner
				.borrow()
				.interface_settings
				.player_settings
				.session_refresh_interval_ms
		{
			tracing::debug!("session refresh interval reached, refreshing");
			self.timings.last_session_refresh = now;
			self.refresh_req = Some(self.session_client.as_ref().unwrap().refresh());
			self.refresh_req
				.as_mut()
				.unwrap()
				.start(&self.inner.borrow().runner_settings.request_wakeup)
				.into_event_error(true)?;
		}

		Ok(())
	}
}
