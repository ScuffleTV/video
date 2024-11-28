#![allow(non_snake_case)]

use std::cell::RefCell;

use js_sys::Promise;
use tokio::sync::mpsc;
use tsify::Tsify;
use wasm_bindgen::prelude::*;

use self::api::ApiClient;
use self::bandwidth::Bandwidth;
use self::inner::{InterfaceSettings, PlayerInner, PlayerInnerHolder, PlayerState, RunnerSettings};
use self::settings::{ErrorWrapper, LoggingLevel, PlayerSettings, PlayerSettingsParsed};
use self::spawn::spawn_runner;
use crate::tracing_wasm::{
	WASMLayerConfig, {self},
};

mod api;
mod bandwidth;
mod errors;
mod events;
mod fetch;
mod inner;
mod runner;
mod settings;
mod spawn;
mod util;
mod variant;

type JsResult<T> = Result<T, JsValue>;
type PlayerResult<T> = Result<T, errors::EventError>;

#[derive(Debug, Clone, serde::Serialize, tsify::Tsify)]
#[tsify(into_wasm_abi)]
pub struct ThumbnailRange {
	pub url: String,
	pub time: f64,
}

#[wasm_bindgen(typescript_custom_section)]
const _: &'static str = r#"
type PlayerEvents = {
    // The player encountered an error.
    // Some errors are fatal and will cause the player to shutdown.
    error: (evt: EventError) => void;

    // The player has started loading a recording or room.
    // You can get the current recording or room by checking the recordingId or roomId property.
    load: () => void;

    // The player has loaded the manifest for the recording or room.
    // The variants property will be populated with the available variants.
    manifestloaded: () => void;

    // The player has switched to a new variant.
    // You can get the new variant id by checking the variantId property.
    // You can also get the actual variant by checking the variants property
    // and indexing it with the variantId.
    variant: (evt: VariantChangeEvent) => void;

    // Low latency mode has been toggled.
    // You can check what the new state is by checking the lowLatency property.
    lowlatency: () => void;

    // ABR has been toggled.
    // You can check what the new state is by checking the abrEnabled property.
    abr: () => void;

    // DVR has been toggled.
    // You can check what the new state is by checking the dvrEnabled property.
    dvr: () => void;

    // Realtime mode has changed.
    // You can check what the new state is by checking the realtimeMode property.
    realtime: () => void;

    // Visibility has changed.
    // You can check what the new state is by checking the visible property.
    visibility: () => void;

    // The player has been destroyed, and will no longer emit events.
    destroyed: () => void;

    // The player has stopped.
    stopped: () => void;

    // The player has started.
    started: () => void;

    // Finished playing the recording or room.
    finished: () => void;
};

// A Scuffle Video Player.
declare class Player {
    // Create a new Player Instance, by providing an organization_id and optionally a server
    // By default the upstream server is the Official Scuffle Video Edge.
    constructor(el: HTMLVideoElement, settings: PlayerSettings);

    // You an provide a room_id to load a room directly.
    // If the room is private you can also provide a token to authenticate.
    loadRoom(room_id: string, token?: string): void;

    // You can provide a recording_id to load a recording directly.
    // If the recording is private you can also provide a token to authenticate.
    loadRecording(recording_id: string, token?: string): void;

    // Get a seek thumbnail for the loaded room or recording.
    // This will return null if the room or recording does not support seek thumbnails.
    // This will also return the closest seek thumbnail to the provided time.
    seekThumbnail(time: number): ThumbnailRange | null;

    // This will stop the player from playing and detach it from the HTMLVideoElement.
    // Any other calls to the player will raise an exception after this is called.
    destroy(): Promise<void>;

    // Stops the player from loading a recording or room.
    // Stopping a player will not detach it from the HTMLVideoElement,
    // and will not reset the player self. Therefore playback sessions,
    // will be resumed when the player is started again.
    stop(): void;

    // Starts the player after it has been stopped.
    start(): void;

    // Seeks the player to realtime
    toRealtime(): void;

    // Allows you to attach event listeners to the player.
    on<K extends keyof PlayerEvents>(event: K, f: PlayerEvents[K]): void;

    // Allows you to remove event listeners from the player.
    removeListener<K extends keyof PlayerEvents>(event: K, f: PlayerEvents[K]): void;

    // Allows you to attach event listeners to the player that will only be called once.
    once<K extends keyof PlayerEvents>(event: K, f: PlayerEvents[K]): void;

    // DVR is enabled by default if the room supports it.
    // DVR allows for rewinding the stream for the duration of the recording.
    // Silently ignores invalid values.
    dvrEnabled: boolean;

    // LowLatency is enabled by default. 
    // Low latency allows for sub second latency, but can cause buffering issues on slower connections.
    // If you are experiencing buffering issues, try disabling this.
    // Silently ignores invalid values.
    lowLatency: boolean;

    // ABR is enabled by default.
    // ABR allows for the player to switch between different quality levels based on your connection.
    // If you are experiencing buffering issues, try disabling this.
    // Silently ignores invalid values.
    abrEnabled: boolean;

    // The current variant id.
    // You can change this value to force the player to switch to a specific variant.
    // This value will be -1 if the player is not currently playing a recording or room.
    // Setting this value will disable ABR.
    // Silently ignores invalid values.
    variantId: number;

    // The next variant id.
    // You can change this value to instruct the player to switch to a specific variant after the next segment.
    // This value will be null if the player is not switching quality levels.
    // Setting this value will disable ABR.
    // Silently ignores invalid values.
    nextVariantId: number | null;

    // The logging level of the player.
    loggingLevel: LoggingLevel;

    // A list of all variants available for the current recording or live room.
    readonly variants: Variant[];

    // The current loaded room id.
    readonly roomId: string | null;

    // The current loaded recording id.
    readonly recordingId: string | null;

    // The amount of bandwidth the player estimates is available.
    readonly bandwidth: number | null;

    // DVR is supported by the current room.
    readonly dvrSupported: boolean;

    // If the player is in realtime mode or not
    readonly realtimeMode: boolean;

    // If the player is currently visible or not.
    readonly visible: boolean;
};
"#;

#[wasm_bindgen(skip_typescript)]
pub struct Player {
	inner: PlayerInnerHolder,
	runner: RefCell<Option<(mpsc::Sender<()>, mpsc::Receiver<()>)>>,
}

const DESTROYED_ERROR: &str = "player has been destroyed, you must create a new instance to continue.";

#[wasm_bindgen]
impl Player {
	#[wasm_bindgen(constructor)]
	pub fn new(el: web_sys::HtmlVideoElement, settings: ErrorWrapper<PlayerSettings>) -> Self {
		let settings: PlayerSettingsParsed = settings.into_inner().into();

		let _logging = tracing_wasm::set_default(WASMLayerConfig::new(settings.logging_level()));

		tracing::debug!(settings = ?settings, "created player");
		let inner = PlayerInner {
			client: ApiClient::new(settings.server.clone(), settings.organization_id),
			runner_settings: RunnerSettings::default(),
			bandwidth: Bandwidth::new(settings.abr_default_bandwidth),
			interface_settings: InterfaceSettings {
				target: None,
				token: None,
				next_variant_id: None,
				player_settings: settings,
				realtime_mode: false,
				auto_seek: false,
				state: PlayerState::Initialized,
			},
			events: events::EventManager::new(),
			video_element: el,
		};

		let inner = PlayerInnerHolder::new(inner);

		let (player_tx, player_rx) = mpsc::channel(1);
		let (runner_tx, runner_rx) = mpsc::channel(1);
		spawn_runner(runner_tx, player_rx, inner.clone());

		Self {
			inner,
			runner: RefCell::new(Some((player_tx, runner_rx))),
		}
	}

	fn destroyed(&self) -> JsResult<()> {
		if self.runner.borrow().is_none() {
			Err(JsValue::from_str(DESTROYED_ERROR))
		} else {
			Ok(())
		}
	}

	#[wasm_bindgen(js_name = loadRoom)]
	pub fn load_room(&self, room_id: String, token: Option<String>) -> JsResult<()> {
		self.destroyed()?;

		tracing_wasm::scope!(self.inner.borrow());

		let Ok(room_id) = room_id.parse() else {
			return Err(JsValue::from_str("invalid room id, room ids must be valid ULIDs"));
		};

		self.inner.borrow_mut().interface_settings.state = PlayerState::Initialized;
		self.inner.borrow_mut().interface_settings.target = Some(inner::VideoTarget::Room(room_id));

		if let Some(token) = token {
			self.inner.borrow_mut().interface_settings.token = Some(token);
		}

		Ok(())
	}

	#[wasm_bindgen(js_name = loadRecording)]
	pub fn load_recording(&self, recording_id: String, token: Option<String>) -> JsResult<()> {
		self.destroyed()?;

		tracing_wasm::scope!(self.inner.borrow());

		let Ok(recording_id) = recording_id.parse() else {
			return Err(JsValue::from_str("invalid recording id, recording ids must be valid ULIDs"));
		};

		self.inner.borrow_mut().interface_settings.state = PlayerState::Initialized;
		self.inner.borrow_mut().interface_settings.target = Some(inner::VideoTarget::Recording(recording_id));

		if let Some(token) = token {
			self.inner.borrow_mut().interface_settings.token = Some(token);
		}

		Ok(())
	}

	#[wasm_bindgen(js_name = destroy)]
	pub fn destroy(&self) -> JsResult<Promise> {
		self.destroyed()?;

		tracing_wasm::scope!(self.inner.borrow());

		self.inner.borrow_mut().interface_settings.state = PlayerState::Shutdown;

		let (_, mut runner) = self.runner.take().unwrap();

		Ok(wasm_bindgen_futures::future_to_promise(async move {
			runner.recv().await;
			Ok(JsValue::undefined())
		}))
	}

	#[wasm_bindgen(js_name = stop)]
	pub fn stop(&self) -> JsResult<()> {
		self.destroyed()?;

		tracing_wasm::scope!(self.inner.borrow());

		self.inner.borrow_mut().interface_settings.state = PlayerState::Stopped;

		Ok(())
	}

	#[wasm_bindgen(js_name = start)]
	pub fn start(&self) -> JsResult<()> {
		self.destroyed()?;

		tracing_wasm::scope!(self.inner.borrow());

		let mut inner = self.inner.borrow_mut();

		match inner.interface_settings.state {
			PlayerState::Initialized | PlayerState::Running => {}
			PlayerState::Stopped => {
				inner.interface_settings.state = PlayerState::Running;
			}
			PlayerState::Shutdown => unreachable!(),
		}

		Ok(())
	}

	#[wasm_bindgen(js_name = on)]
	pub fn on(&self, event: String, f: JsValue) -> JsResult<()> {
		self.destroyed()?;

		tracing_wasm::scope!(self.inner.borrow());

		let Ok(ty) = event.parse() else {
			return Err(JsValue::from_str(format!("invalid event type: {event}").as_str()));
		};

		self.inner.borrow_mut().events.add_event_listener(ty, f, false);

		Ok(())
	}

	#[wasm_bindgen(js_name = removeListener)]
	pub fn remove_listener(&self, event: String, f: JsValue) -> JsResult<()> {
		self.destroyed()?;

		tracing_wasm::scope!(self.inner.borrow());

		let Ok(ty) = event.parse() else {
			return Err(JsValue::from_str(format!("invalid event type: {event}").as_str()));
		};

		self.inner.borrow_mut().events.remove_event_listener(ty, f);

		Ok(())
	}

	#[wasm_bindgen(js_name = off)]
	pub fn off(&self, event: String, f: JsValue) -> JsResult<()> {
		self.destroyed()?;

		tracing_wasm::scope!(self.inner.borrow());

		let Ok(ty) = event.parse() else {
			return Err(JsValue::from_str(format!("invalid event type: {event}").as_str()));
		};

		self.inner.borrow_mut().events.remove_event_listener(ty, f);

		Ok(())
	}

	#[wasm_bindgen(js_name = once)]
	pub fn once(&self, event: String, f: JsValue) -> JsResult<()> {
		self.destroyed()?;

		tracing_wasm::scope!(self.inner.borrow());

		let Ok(ty) = event.parse() else {
			return Err(JsValue::from_str(format!("invalid event type: {event}").as_str()));
		};

		self.inner.borrow_mut().events.add_event_listener(ty, f, true);

		Ok(())
	}

	#[wasm_bindgen(setter = dvrEnabled)]
	pub fn set_dvr_enabled(&self, dvr_enabled: bool) -> JsResult<()> {
		self.destroyed()?;

		tracing_wasm::scope!(self.inner.borrow());

		self.inner.borrow_mut().interface_settings.player_settings.enable_dvr = dvr_enabled;

		events::dispatch!(self.inner.borrow_mut().events.emit(events::UserEvent::Dvr));

		Ok(())
	}

	#[wasm_bindgen(getter = dvrEnabled)]
	pub fn dvr_enabled(&self) -> JsResult<bool> {
		self.destroyed()?;

		tracing_wasm::scope!(self.inner.borrow());

		Ok(self.inner.borrow().interface_settings.player_settings.enable_dvr)
	}

	#[wasm_bindgen(getter = dvrSupported)]
	pub fn dvr_supported(&self) -> JsResult<bool> {
		self.destroyed()?;

		tracing_wasm::scope!(self.inner.borrow());

		if self.inner.borrow().interface_settings.state == PlayerState::Initialized {
			return Ok(false);
		}

		Ok(self.inner.borrow().runner_settings.dvr_supported)
	}

	#[wasm_bindgen(getter = visible)]
	pub fn visible(&self) -> JsResult<bool> {
		self.destroyed()?;

		tracing_wasm::scope!(self.inner.borrow());

		Ok(self.inner.borrow().runner_settings.visible)
	}

	#[wasm_bindgen(js_name = toRealtime)]
	pub fn to_realtime(&self) -> JsResult<()> {
		self.destroyed()?;

		tracing_wasm::scope!(self.inner.borrow());

		if self.inner.borrow().interface_settings.state != PlayerState::Running {
			return Err(JsValue::from_str("realtime mode can only be set while the player is running"));
		}

		let updated = self.inner.borrow_mut().set_realtime(true).map_err(JsValue::from_str)?;

		self.inner.borrow_mut().interface_settings.auto_seek = true;

		if updated {
			events::dispatch!(self.inner.borrow_mut().events.emit(events::UserEvent::Realtime));
		}

		Ok(())
	}

	#[wasm_bindgen(getter = realtimeMode)]
	pub fn realtime_mode(&self) -> JsResult<Option<bool>> {
		self.destroyed()?;

		tracing_wasm::scope!(self.inner.borrow());

		if self.inner.borrow().interface_settings.state != PlayerState::Running {
			return Ok(None);
		}

		Ok(Some(self.inner.borrow().interface_settings.realtime_mode))
	}

	#[wasm_bindgen(setter = lowLatency)]
	pub fn set_low_latency(&self, low_latency: bool) -> JsResult<()> {
		self.destroyed()?;

		tracing_wasm::scope!(self.inner.borrow());

		self.inner.borrow_mut().interface_settings.player_settings.enable_low_latency = low_latency;

		events::dispatch!(self.inner.borrow_mut().events.emit(events::UserEvent::LowLatency));

		Ok(())
	}

	#[wasm_bindgen(getter = lowLatency)]
	pub fn low_latency(&self) -> JsResult<bool> {
		self.destroyed()?;

		tracing_wasm::scope!(self.inner.borrow());

		Ok(self.inner.borrow().interface_settings.player_settings.enable_low_latency)
	}

	#[wasm_bindgen(setter = abrEnabled)]
	pub fn set_abr_enabled(&self, abr_enabled: bool) -> JsResult<()> {
		self.destroyed()?;

		tracing_wasm::scope!(self.inner.borrow());

		self.inner.borrow_mut().interface_settings.player_settings.enable_abr = abr_enabled;

		events::dispatch!(self.inner.borrow_mut().events.emit(events::UserEvent::Abr));

		Ok(())
	}

	#[wasm_bindgen(getter = abrEnabled)]
	pub fn abr_enabled(&self) -> JsResult<bool> {
		self.destroyed()?;

		tracing_wasm::scope!(self.inner.borrow());

		Ok(self.inner.borrow().interface_settings.player_settings.enable_abr)
	}

	#[wasm_bindgen(setter = variantId)]
	pub fn set_variant_id(&self, variant_id: i32) -> JsResult<()> {
		self.destroyed()?;

		tracing_wasm::scope!(self.inner.borrow());

		if variant_id == -1 {
			self.set_abr_enabled(true)?;
		} else if variant_id >= 0 {
			self.set_abr_enabled(false)?;
			self.inner.borrow_mut().interface_settings.next_variant_id = Some(inner::NextVariant::Force(variant_id as u32));
		}

		Ok(())
	}

	#[wasm_bindgen(setter = nextVariantId)]
	pub fn set_next_variant_id(&self, variant_id: i32) -> JsResult<()> {
		self.destroyed()?;

		tracing_wasm::scope!(self.inner.borrow());

		if variant_id == -1 {
			self.set_abr_enabled(true)?;
		} else if variant_id >= 0 {
			self.set_abr_enabled(false)?;
			self.inner.borrow_mut().interface_settings.next_variant_id = Some(inner::NextVariant::Switch(variant_id as u32));
		}

		Ok(())
	}

	#[wasm_bindgen(getter = variantId)]
	pub fn variant_id(&self) -> JsResult<i32> {
		self.destroyed()?;

		tracing_wasm::scope!(self.inner.borrow());

		if self.inner.borrow().interface_settings.state == PlayerState::Initialized {
			return Ok(-1);
		}

		Ok(self.inner.borrow().runner_settings.current_variant_id as i32)
	}

	#[wasm_bindgen(getter = nextVariantId)]
	pub fn next_variant_id(&self) -> JsResult<Option<i32>> {
		self.destroyed()?;

		tracing_wasm::scope!(self.inner.borrow());

		Ok(self.inner.borrow().interface_settings.next_variant_id.map(|next| match next {
			inner::NextVariant::Switch(id) => id as i32,
			inner::NextVariant::Force(id) => id as i32,
			inner::NextVariant::Auto { id, .. } => id as i32,
		}))
	}

	#[wasm_bindgen(getter = variants)]
	pub fn variants(&self) -> JsResult<js_sys::Array> {
		self.destroyed()?;

		tracing_wasm::scope!(self.inner.borrow());

		Ok(self
			.inner
			.borrow()
			.runner_settings
			.variants
			.iter()
			.map(|variant| variant.into_js().unwrap())
			.collect())
	}

	#[wasm_bindgen(getter = roomId)]
	pub fn room_id(&self) -> JsResult<Option<String>> {
		self.destroyed()?;

		tracing_wasm::scope!(self.inner.borrow());

		Ok(self
			.inner
			.borrow()
			.interface_settings
			.target
			.as_ref()
			.and_then(|target| match target {
				inner::VideoTarget::Room(id) => Some(id.to_string()),
				_ => None,
			}))
	}

	#[wasm_bindgen(getter = recordingId)]
	pub fn recording_id(&self) -> JsResult<Option<String>> {
		self.destroyed()?;

		tracing_wasm::scope!(self.inner.borrow());

		Ok(self
			.inner
			.borrow()
			.interface_settings
			.target
			.as_ref()
			.and_then(|target| match target {
				inner::VideoTarget::Recording(id) => Some(id.to_string()),
				_ => None,
			}))
	}

	#[wasm_bindgen(js_name = seekThumbnail)]
	pub fn seek_thumbnail(&self, time: f64) -> JsResult<Option<ThumbnailRange>> {
		self.destroyed()?;

		tracing_wasm::scope!(self.inner.borrow());

		let inner = self.inner.borrow();

		if inner.runner_settings.thumbnails.is_empty() || inner.runner_settings.thumbnail_prefix.is_none() {
			return Ok(None);
		}

		let prefix = inner.runner_settings.thumbnail_prefix.as_ref().unwrap();

		let r = inner
			.runner_settings
			.thumbnails
			.binary_search_by(|thumbnail| thumbnail.start_time.partial_cmp(&time).unwrap());

		match r {
			Ok(idx) => {
				let thumbnail = &inner.runner_settings.thumbnails[idx];
				Ok(Some(ThumbnailRange {
					time: thumbnail.start_time,
					url: format!("{prefix}/{}", thumbnail.id),
				}))
			}
			Err(idx) => {
				let before = inner
					.runner_settings
					.thumbnails
					.get(idx.saturating_sub(1))
					.map(|t| (t.start_time - time).abs());
				let first = inner.runner_settings.thumbnails.get(idx).map(|t| (t.start_time - time).abs());
				let second = inner
					.runner_settings
					.thumbnails
					.get(idx.saturating_add(1))
					.map(|t| (t.start_time - time).abs());

				// Which one is the closest?
				let (closest, _) = [before, first, second]
					.iter()
					.enumerate()
					.fold((0, time), |(idx, min), (i, v)| {
						let Some(v) = *v else {
							return (idx, min);
						};

						if v < min { (i, v) } else { (idx, min) }
					});

				let thumbnail = &inner.runner_settings.thumbnails[(idx + closest).saturating_sub(1)];
				Ok(Some(ThumbnailRange {
					url: format!("{prefix}/{}", thumbnail.id),
					time: thumbnail.start_time,
				}))
			}
		}
	}

	#[wasm_bindgen(getter = bandwidth)]
	pub fn bandwidth(&self) -> JsResult<Option<f64>> {
		self.destroyed()?;

		tracing_wasm::scope!(self.inner.borrow());

		Ok(Some(self.inner.borrow().bandwidth.estimate()))
	}

	#[wasm_bindgen(setter = loggingLevel)]
	pub fn set_logging_level(&self, level: LoggingLevel) -> JsResult<()> {
		self.destroyed()?;

		tracing_wasm::scope!(self.inner.borrow());

		self.inner.borrow_mut().interface_settings.player_settings.logging_level = level;

		Ok(())
	}

	#[wasm_bindgen(getter = loggingLevel)]
	pub fn logging_level(&self) -> JsResult<LoggingLevel> {
		self.destroyed()?;

		tracing_wasm::scope!(self.inner.borrow());

		Ok(self.inner.borrow().interface_settings.player_settings.logging_level)
	}
}
