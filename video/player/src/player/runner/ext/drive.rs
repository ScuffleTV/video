use crate::player::errors::{ErrorCode, EventError, EventErrorExtFetch};
use crate::player::inner::{PlayerState, VideoTarget};
use crate::player::runner::track::TrackResult;
use crate::player::runner::utils::{make_media_source_holder, MediaSourceEvent};
use crate::player::runner::{source_buffer, Runner};
use crate::player::{events, PlayerResult};

impl Runner {
	async fn drive_audio_track(&mut self, idx: usize) {
		let next_audio_track_start = if idx == self.active_audio_track_idx {
			self.next_audio_track_idx
				.and_then(|idx| self.audio_tracks[idx].will_start_at())
		} else {
			None
		};

		loop {
			let audio_track = &mut self.audio_tracks[idx];

			// We only check if the audio track is finished, because if one is finished,
			// they all are.
			if audio_track.finished() && !self.flags.is_finished {
				self.flags.is_finished = true;

				let (start, end) = audio_track.seekable_range();
				self.mediasource
					.as_ref()
					.unwrap()
					.set_live_seekable_range(start, end)
					.unwrap();
				self.mediasource.as_ref().unwrap().set_duration(end);
				events::dispatch!(self.inner.borrow_mut().events.emit(events::UserEvent::Finished));

				let dvr_supported = self.inner.borrow().use_dvr(false);

				if dvr_supported {
					tracing::debug!("dvr is supported, disabling realtime");
					self.inner.borrow_mut().runner_settings.realtime_supported = false;
					self.set_realtime_mode(false);
				} else {
					tracing::debug!("dvr is not supported, stopping");
					self.inner.borrow_mut().interface_settings.state = PlayerState::Stopped;
					events::dispatch!(self.inner.borrow_mut().events.emit(events::UserEvent::Stopped));
				}

				continue;
			}

			match audio_track.drive(&self.inner, self.timings.current_player_time) {
				Err(err) => {
					if err.fatal {
						tracing::debug!("audio track {idx} errored with fatal, stopping");
						self.inner.borrow_mut().interface_settings.state = PlayerState::Stopped;
					}

					events::dispatch!(self.inner.borrow_mut().events.emit(events::UserEvent::Error(err)));

					return;
				}
				Ok(Some(TrackResult::Media {
					data,
					decode_time,
					duration,
					start_time,
					end_time,
				})) => {
					if self.active_audio_track_idx != idx {
						tracing::trace!(start_time, end_time, "adding audio data to temporary buffer");
						self.temporary_audio_buffer.push((data, decode_time, duration));
						continue;
					} else if let Some(next_start) = next_audio_track_start {
						if start_time >= next_start {
							// We are ready to switch to the next track.
							audio_track.stop();

							tracing::trace!(
								"audio track {idx} finished, switching to {next_idx}",
								idx = idx,
								next_idx = self.next_audio_track_idx.unwrap()
							);

							tracing::debug!("removing audio track {idx} from {start_time} to {next_start}");
							self.source_buffers
								.as_mut()
								.unwrap()
								.audio
								.remove(start_time, next_start)
								.await
								.unwrap();

							tracing::trace!("switching to audio track {idx}", idx = self.next_audio_track_idx.unwrap());
							self.active_audio_track_idx = self.next_audio_track_idx.take().unwrap();

							self.init_audio().await;

							self.audio_tracks[self.active_audio_track_idx].start();

							for (data, decode_time, duration) in self.temporary_audio_buffer.drain(..) {
								tracing::trace!(decode_time, duration, "appending audio from temporary buffer");
								self.source_buffers
									.as_mut()
									.unwrap()
									.audio
									.append_buffer(data.into())
									.await
									.unwrap();
								if let Some(video_factory) = self.video_factory.as_mut() {
									let data = video_factory.media_segment(decode_time, duration);
									tracing::trace!(decode_time, duration, "appending video from video factory");
									self.source_buffers
										.as_mut()
										.unwrap()
										.video
										.append_buffer(data.into())
										.await
										.unwrap();
								}
							}

							return;
						}
					}

					self.init_audio().await;

					tracing::debug!(start_time, end_time, "appending audio data");
					self.source_buffers
						.as_mut()
						.unwrap()
						.audio
						.append_buffer(data.into())
						.await
						.unwrap();

					if self.video_factory.is_some() {
						self.init_video().await;
					}

					if let Some(video_factory) = self.video_factory.as_mut() {
						tracing::trace!(start_time, end_time, "appending video from video factory");
						let data = video_factory.media_segment(decode_time, duration);
						self.source_buffers
							.as_mut()
							.unwrap()
							.video
							.append_buffer(data.into())
							.await
							.unwrap();

						if let Some(stop_at) = video_factory.will_stop_at() {
							if self.next_video_track_idx.is_none() {
								video_factory.start();
							} else if start_time >= stop_at {
								tracing::trace!("stopping video factory");
								self.video_factory = None;
								self.active_video_track_idx = Some(self.next_video_track_idx.take().unwrap());
								self.video_tracks[self.active_video_track_idx.unwrap()].start();
								self.init_video().await;

								for data in self.temporary_video_buffer.drain(..) {
									tracing::trace!("appending video from temporary buffer");
									self.source_buffers
										.as_mut()
										.unwrap()
										.video
										.append_buffer(data.into())
										.await
										.unwrap();
								}
							}
						}
					}
				}
				Ok(Some(TrackResult::Discontinuity(start, end))) => {
					if self.inner.borrow().interface_settings.realtime_mode {
						tracing::error!("audio discontinuity from {start} to {end}, seeking to {end}");
						self.element.set_current_time(end + 0.1);
						self.timings.current_player_time = end + 0.1;
					} else if !self.element.paused() {
						tracing::error!("audio discontinuity from {start} to {end}, pausing");
						self.element.pause().ok();
					}
					return;
				}
				Ok(None) => {
					return;
				}
			}
		}
	}

	async fn drive_video_track(&mut self, idx: usize) {
		let next_video_track_start = if Some(idx) == self.active_video_track_idx {
			self.next_video_track_idx
				.and_then(|idx| self.video_tracks[idx].will_start_at())
		} else {
			None
		};

		loop {
			let video_track = &mut self.video_tracks[idx];

			match video_track.drive(&self.inner, self.timings.current_player_time) {
				Err(err) => {
					if err.fatal {
						tracing::debug!("video track {idx} errored with fatal, stopping");
						self.inner.borrow_mut().interface_settings.state = PlayerState::Stopped;
					}

					events::dispatch!(self.inner.borrow_mut().events.emit(events::UserEvent::Error(err)));

					return;
				}
				Ok(Some(TrackResult::Media {
					data,
					start_time,
					end_time,
					..
				})) => {
					if self.active_video_track_idx != Some(idx) {
						tracing::trace!(start_time, end_time, "adding video data to temporary buffer");
						self.temporary_video_buffer.push(data);
						if let Some(video_factory) = self.video_factory.as_mut() {
							if video_factory.will_stop_at().is_none() {
								tracing::trace!("stopping video factory at {start_time}");
								video_factory.stop_at(start_time);
							}
						}
						continue;
					} else if let Some(next_start) = next_video_track_start {
						if start_time >= next_start {
							tracing::trace!(
								"video track {idx} finished, switching to {next_idx}",
								next_idx = self.next_video_track_idx.unwrap()
							);

							// We are ready to switch to the next track.
							video_track.stop();

							tracing::debug!("removing video track {idx} from {start_time} to {next_start}");
							self.source_buffers
								.as_mut()
								.unwrap()
								.video
								.remove(start_time, next_start)
								.await
								.unwrap();

							tracing::trace!("switching to video track {idx}", idx = self.next_video_track_idx.unwrap());
							self.active_video_track_idx = Some(self.next_video_track_idx.take().unwrap());
							self.video_tracks[self.active_video_track_idx.unwrap()].start();

							self.init_video().await;

							for data in self.temporary_video_buffer.drain(..) {
								tracing::trace!("appending video from temporary buffer");
								self.source_buffers
									.as_mut()
									.unwrap()
									.video
									.append_buffer(data.into())
									.await
									.unwrap();
							}
							return;
						}
					}

					self.init_video().await;

					tracing::debug!(start_time, end_time, "appending video data");
					self.source_buffers
						.as_mut()
						.unwrap()
						.video
						.append_buffer(data.into())
						.await
						.unwrap();
				}
				Ok(Some(TrackResult::Discontinuity(start, end))) => {
					if self.inner.borrow().interface_settings.realtime_mode {
						tracing::error!("video discontinuity from {start} to {end}, seeking to {end}");
						self.element.set_current_time(end + 0.1);
						self.timings.current_player_time = end + 0.1;
					} else if !self.element.paused() {
						tracing::error!("video discontinuity from {start} to {end}, pausing");
						self.element.pause().ok();
					}
					return;
				}
				Ok(None) => {
					return;
				}
			}
		}
	}

	fn drive_session_refresh(&mut self) -> PlayerResult<()> {
		if let Some(req) = self.refresh_req.as_mut() {
			if let Some(result) = req
				.json(&self.inner.borrow().runner_settings.request_wakeup)
				.into_event_error(true)?
			{
				if !result.success {
					return Err(EventError::new(ErrorCode::Other, "session refresh failed".into(), true));
				}
			}
		}

		Ok(())
	}

	pub async fn drive_running(&mut self, now: f64) {
		if self.flags.is_stopped {
			tracing::trace!("starting runner");
			self.flags.is_stopped = false;
			events::dispatch!(self.inner.borrow_mut().events.emit(events::UserEvent::Started));
		}

		self.timings.last_drive = now;
		self.timings.current_player_time = self.element.current_time();

		// If we are in realtime we want to keep the buffer around the target duration.
		self.handle_rate_control(now);
		// We want to make sure we arent stopped due to a buffer hole.
		self.handle_buffer_hole(now);
		// We want to calculate the bandwidth of the player.
		self.handle_bandwidth(now);

		// We want to handle the session refresh.
		if let Err(err) = self.handle_session_refresh(now) {
			let fatal = err.fatal;
			if fatal {
				self.inner.borrow_mut().interface_settings.state = PlayerState::Stopped;
			}

			events::dispatch!(self.inner.borrow_mut().events.emit(events::UserEvent::Error(err)));

			if fatal {
				return;
			}
		}

		if let Err(err) = self.drive_session_refresh() {
			let fatal = err.fatal;
			if fatal {
				self.inner.borrow_mut().interface_settings.state = PlayerState::Stopped;
			}

			events::dispatch!(self.inner.borrow_mut().events.emit(events::UserEvent::Error(err)));

			if fatal {
				return;
			}
		}

		if let Err(err) = self.handle_buffer_size().await {
			let fatal = err.fatal;
			if fatal {
				self.inner.borrow_mut().interface_settings.state = PlayerState::Stopped;
			}

			events::dispatch!(self.inner.borrow_mut().events.emit(events::UserEvent::Error(err)));

			if fatal {
				return;
			}
		}

		if self.flags.is_finished && self.mediasource().unwrap().duration() - self.timings.current_player_time < 0.25 {
			return;
		}

		self.handle_next_variant();

		if let Some(next_audio_track_id) = self.next_audio_track_idx {
			tracing::trace!(next_audio_track_id, "driving next audio track");
			self.drive_audio_track(next_audio_track_id).await;
		}

		if let Some(next_video_track_id) = self.next_video_track_idx {
			tracing::trace!(next_video_track_id, "driving next video track");
			self.drive_video_track(next_video_track_id).await;
		}

		self.drive_audio_track(self.active_audio_track_idx).await;
		let (mut start_time, mut end_time) = self.audio_tracks[self.active_audio_track_idx].seekable_range();

		if let Some(video_track_idx) = self.active_video_track_idx {
			self.drive_video_track(video_track_idx).await;

			let (video_start_time, video_end_time) = self.video_tracks[video_track_idx].seekable_range();
			start_time = start_time.max(video_start_time);
			end_time = end_time.min(video_end_time);
		}

		if !self.flags.is_finished && start_time < end_time {
			self.mediasource()
				.unwrap()
				.set_live_seekable_range(start_time, end_time)
				.unwrap();
			tracing::trace!(start_time, end_time, "setting live seekable range");
		}

		self.handle_autoseek();
	}

	pub async fn drive_shutdown(&mut self) {
		// We don't have to do anything.
		if !self.flags.is_stopped {
			self.flags.is_stopped = true;
			events::dispatch!(self.inner.borrow_mut().events.emit(events::UserEvent::Stopped));
		}
	}

	pub async fn drive_init(&mut self) {
		if self.inner.borrow().interface_settings.target.is_none() {
			return;
		}

		self.flags.is_stopped = true;

		events::dispatch!(self.inner.borrow_mut().events.emit(events::UserEvent::LoadStart));

		let target = self.inner.borrow().interface_settings.target.unwrap();

		if let Err(err) = match target {
			VideoTarget::Room(room_id) => self.load_room(room_id).await,
			VideoTarget::Recording(recording_id) => self.load_recording(recording_id).await,
		} {
			self.inner.borrow_mut().interface_settings.target = None;
			events::dispatch!(self.inner.borrow_mut().events.emit(events::UserEvent::Error(err)));
		} else {
			let mut mediasource = make_media_source_holder();

			if let Some(url) = self.player_url.take() {
				web_sys::Url::revoke_object_url(&url).unwrap();
			}

			self.player_url = Some(web_sys::Url::create_object_url_with_source(&mediasource).unwrap());

			self.element.set_src(self.player_url.as_deref().unwrap());

			match mediasource.events().recv().await {
				Some(MediaSourceEvent::SourceOpen) => {}
				None | Some(MediaSourceEvent::SourceClose) => {
					events::dispatch!(self.inner.borrow_mut().events.emit(events::UserEvent::Error(EventError::new(
						ErrorCode::Other,
						"Media source closed before it could be opened".into(),
						true,
					))));
					return;
				}
			}

			match target {
				VideoTarget::Room(_) => {
					mediasource.set_duration(f64::INFINITY);
				}
				VideoTarget::Recording(_) => {
					mediasource.set_duration(0.0);
				}
			}

			self.mediasource = Some(mediasource);
			self.source_buffers = Some(source_buffer::SourceBuffers::new(self.mediasource().unwrap()));
			self.audio_init = None;
			self.video_init = None;
			self.audio_only_auto_switch = None;
			self.temporary_audio_buffer.clear();
			self.temporary_video_buffer.clear();
			self.next_audio_track_idx = None;
			self.next_video_track_idx = None;
			self.video_factory = None;

			self.inner.borrow_mut().interface_settings.next_variant_id = None;
		}
	}
}
