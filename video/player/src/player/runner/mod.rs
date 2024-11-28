use bytes::Bytes;
use video_player_types::{RoomPlaylistTrackAudio, RoomPlaylistTrackVideo, SessionRefresh};
use web_sys::MediaSource;

use self::blank::VideoFactory;
use self::track::TrackState;
use self::utils::{make_video_element_holder, MediaSourceEvent, VideoElementEvent};
use self::visibility_detector::VisibilityDetector;
use super::api::{ApiMediaClient, ApiSessionClient, Json};
use super::inner::{PlayerInnerHolder, PlayerState};
use super::util::{now, Holder};

mod blank;
mod ext;
mod flags;
mod source_buffer;
mod track;
mod utils;
mod visibility_detector;

pub struct Runner {
	inner: PlayerInnerHolder,

	/// The current value of the audio source buffer.
	audio_init: Option<usize>,
	/// The current value of the video source buffer.
	video_init: Option<Option<usize>>,

	/// A session client used to fetch manifests.
	session_client: Option<ApiSessionClient>,
	/// A media client used to fetch media segments.
	media_client: Option<ApiMediaClient>,

	/// A list of audio tracks.
	audio_tracks: Vec<TrackState<RoomPlaylistTrackAudio>>,
	/// A list of video tracks.
	video_tracks: Vec<TrackState<RoomPlaylistTrackVideo>>,

	/// A video factory is used to generate blank video frames when there is no
	/// video track.
	video_factory: Option<VideoFactory>,
	/// The current active video track index.
	active_video_track_idx: Option<usize>,
	/// The current active audio track index.
	active_audio_track_idx: usize,

	/// The next video track index to switch to.
	next_video_track_idx: Option<usize>,
	/// The next audio track index to switch to.
	next_audio_track_idx: Option<usize>,

	/// This was the variant id before we switched to audio only, automatically.
	audio_only_auto_switch: Option<u32>,

	/// The media source.
	mediasource: Option<Holder<MediaSource, MediaSourceEvent>>,

	/// The video element.
	element: Holder<web_sys::HtmlVideoElement, VideoElementEvent>,

	/// A temporary buffer for video frames, this is used when we are switching
	/// video tracks.
	temporary_video_buffer: Vec<Bytes>,

	/// A temporary buffer for audio frames, this is used when we are switching
	/// audio tracks. We also need to store the timestamp and duration of the
	/// audio frame.
	temporary_audio_buffer: Vec<(Bytes, u64, u32)>,

	/// A holder for the source buffers.
	source_buffers: Option<source_buffer::SourceBuffers>,

	/// The visibility detector is used to detect if the video element is
	/// visible or not.
	visibility_detector: VisibilityDetector,

	/// The current timing state of the player.
	timings: flags::Timings,
	/// The current flags state of the player.
	flags: flags::Flags,
	/// Current playback factor.
	playback_factor: f64,

	refresh_req: Option<Json<SessionRefresh>>,

	player_url: Option<String>,
}

impl Runner {
	pub fn new(inner: PlayerInnerHolder) -> Self {
		let el = inner.borrow().video_element.clone();
		let element = make_video_element_holder(el.clone());

		Self {
			inner,

			video_factory: None,

			active_video_track_idx: Some(0),
			active_audio_track_idx: 0,

			audio_init: None,
			video_init: None,

			next_audio_track_idx: None,
			next_video_track_idx: None,

			audio_tracks: Vec::new(),
			video_tracks: Vec::new(),

			temporary_audio_buffer: Vec::new(),
			temporary_video_buffer: Vec::new(),

			session_client: None,
			media_client: None,

			source_buffers: None,
			mediasource: None,
			element,

			playback_factor: el.playback_rate(),

			audio_only_auto_switch: None,

			visibility_detector: VisibilityDetector::new(el),

			timings: Default::default(),
			flags: flags::Flags {
				is_finished: false,
				is_stopped: true,
			},
			player_url: None,
			refresh_req: None,
		}
	}

	pub fn inner(&self) -> &PlayerInnerHolder {
		&self.inner
	}

	pub fn mediasource(&mut self) -> Option<&mut Holder<MediaSource, MediaSourceEvent>> {
		self.mediasource.as_mut()
	}

	pub async fn drive(&mut self) {
		let now = now();

		// We want to handle visibility changes.
		self.handle_visibility(now);

		self.handle_video_events(now);

		// We do this to prevent the player from seeking too often.
		// But we also dont want to prevent the player from loading for too long.
		if now - self.timings.last_seeked
			< self
				.inner
				.borrow()
				.interface_settings
				.player_settings
				.seeked_debounce_threshold_ms
			&& now - self.timings.last_drive
				< self
					.inner
					.borrow()
					.interface_settings
					.player_settings
					.player_drive_cooldown_ms
		{
			return;
		}

		let state = { self.inner.borrow().interface_settings.state };
		match state {
			PlayerState::Shutdown | PlayerState::Stopped => {
				self.drive_shutdown().await;
			}
			PlayerState::Initialized => {
				self.drive_init().await;
			}
			PlayerState::Running => {
				self.drive_running(now).await;
			}
		}
	}

	pub fn shutdown(&mut self) {
		if let Some(url) = self.player_url.take() {
			web_sys::Url::revoke_object_url(&url).unwrap();

			if self.element.src() == url {
				self.element.set_src("");
				self.element.load();
			}
		}
	}
}
