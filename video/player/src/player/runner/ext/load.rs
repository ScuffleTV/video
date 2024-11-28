use ulid::Ulid;
use video_player_types::SessionPlaylist;
use web_sys::MediaSource;

use crate::player::errors::{ErrorCode, EventError, EventErrorExtFetch};
use crate::player::events::{
	VariantEvent, {self},
};
use crate::player::inner::PlayerState;
use crate::player::runner::track::TrackState;
use crate::player::runner::Runner;
use crate::player::{variant, PlayerResult};

impl Runner {
	fn handle_resp(&mut self, mut resp: SessionPlaylist) -> PlayerResult<()> {
		resp.audio_tracks.retain(|v| {
			let codec = format!("audio/mp4; codecs=\"{}\"", v.codec);
			let supported = MediaSource::is_type_supported(&codec);

			tracing::debug!(codec, "audio track is supported: {supported}");

			supported
		});

		resp.video_tracks.retain(|v| {
			let codec = format!("video/mp4; codecs=\"{}\"", v.codec);
			let supported = MediaSource::is_type_supported(&codec);

			tracing::debug!(codec, "video track is supported: {supported}");

			supported
		});

		if resp.audio_tracks.is_empty() {
			return Err(EventError::new(ErrorCode::Other, "no supported audio tracks".into(), true));
		}

		self.video_tracks.clear();
		self.audio_tracks.clear();

		for video in resp.video_tracks.iter() {
			self.video_tracks.push(TrackState::new(
				self.session_client.clone().unwrap(),
				self.media_client.clone(),
				video.clone(),
			));
		}

		let mut variants = Vec::new();
		for (audio_id, audio) in resp.audio_tracks.iter().enumerate() {
			self.audio_tracks.push(TrackState::new(
				self.session_client.clone().unwrap(),
				self.media_client.clone(),
				audio.clone(),
			));
			variants.push(variant::Variant::new((audio_id, audio), None));
			for (video_id, video) in self.video_tracks.iter().enumerate() {
				variants.push(variant::Variant::new((audio_id, audio), Some((video_id, video.track()))));
			}
		}

		// todo this is wrong, its not always the highest bitrate = highest quality
		variants
			.sort_unstable_by_key(|s| s.audio_track.bitrate + s.video_track.as_ref().map(|v| v.bitrate).unwrap_or_default());

		// Sort by bitrate high to low.
		variants.reverse();

		self.inner.borrow_mut().runner_settings.variants = variants;
		self.inner.borrow_mut().interface_settings.state = PlayerState::Running;

		let variant_id = if self.inner.borrow().interface_settings.player_settings.enable_abr {
			self.abr_variant_id().unwrap_or(0)
		} else {
			0
		};

		let (audio_id, video_id) = self
			.inner
			.borrow()
			.runner_settings
			.variants
			.get(variant_id as usize)
			.map(|v| (v.audio_track.id, v.video_track.as_ref().map(|t| t.id)))
			.unwrap();

		self.active_audio_track_idx = audio_id;
		self.active_video_track_idx = video_id;

		tracing::debug!(variant_id, "setting variant id");
		self.timings.last_abr_switch = -1.0;

		self.inner.borrow_mut().runner_settings.current_variant_id = variant_id;

		events::dispatch!(self.inner.borrow_mut().events.emit(events::UserEvent::ManifestLoaded));

		events::dispatch!(self.inner.borrow_mut().events.emit(events::UserEvent::Variant(VariantEvent {
			automatic: None,
			variant_id,
			previous_variant_id: -1,
		})));

		Ok(())
	}

	pub async fn load_room(&mut self, room_id: Ulid) -> PlayerResult<()> {
		self.flags.reset();

		self.timings.reset();

		let mut room_req = self
			.inner
			.borrow()
			.client
			.get_room(room_id, self.inner.borrow().interface_settings.token.as_deref());

		let wakeup = self.inner.borrow().runner_settings.request_wakeup.clone();

		tracing::debug!("fetching room playlist");

		let resp = room_req.wait_json(&wakeup).await.into_event_error(true)?;

		self.session_client = Some(self.inner.borrow().client.session_client(&resp));
		self.media_client = Some(self.inner.borrow().client.media_client(room_id));

		self.handle_resp(resp)?;

		self.inner.borrow_mut().interface_settings.auto_seek = self.element.current_time() == -1.0;
		self.inner.borrow_mut().runner_settings.realtime_supported = true;
		self.set_realtime_mode(true);

		Ok(())
	}

	pub async fn load_recording(&mut self, recording_id: Ulid) -> PlayerResult<()> {
		self.flags.reset();
		self.timings.reset();

		let mut recording_req = self
			.inner
			.borrow()
			.client
			.get_recording(recording_id, self.inner.borrow().interface_settings.token.as_deref());

		let wakeup = self.inner.borrow().runner_settings.request_wakeup.clone();

		tracing::debug!("fetching recording playlist");

		let resp = recording_req.wait_json(&wakeup).await.into_event_error(true)?;

		self.session_client = Some(self.inner.borrow().client.session_client(&resp));

		self.media_client = None;

		self.handle_resp(resp)?;

		self.inner.borrow_mut().interface_settings.auto_seek = self.element.current_time() == -1.0;
		self.inner.borrow_mut().runner_settings.realtime_supported = false;
		self.inner.borrow_mut().runner_settings.dvr_supported = true;
		self.set_realtime_mode(false);

		Ok(())
	}
}
