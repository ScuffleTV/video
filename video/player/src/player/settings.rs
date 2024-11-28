use serde::Deserialize;
use ulid::Ulid;
use url::Url;
use wasm_bindgen::JsValue;

#[derive(tsify::Tsify, Debug, Default, serde::Deserialize, serde::Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[tsify(from_wasm_abi, into_wasm_abi)]
pub enum LoggingLevel {
	#[default]
	Info,
	Trace,
	Debug,
	Warn,
	Error,
}

fn deserialize_ulid<'de, D>(deserializer: D) -> Result<Ulid, D::Error>
where
	D: serde::Deserializer<'de>,
{
	// Try from string first
	let deserialized_str = String::deserialize(deserializer)?;
	let ulid =
		Ulid::from_string(&deserialized_str).map_err(|err| serde::de::Error::custom(format!("invalid ulid: {err}")))?;
	Ok(ulid)
}

#[derive(tsify::Tsify)]
pub struct ErrorWrapper<T> {
	inner: T,
}

impl<T: tsify::Tsify> wasm_bindgen::describe::WasmDescribe for ErrorWrapper<T>
where
	<T as tsify::Tsify>::JsType: wasm_bindgen::describe::WasmDescribe,
{
	fn describe() {
		<T as tsify::Tsify>::JsType::describe();
	}
}

impl<T: serde::de::DeserializeOwned + tsify::Tsify> wasm_bindgen::convert::FromWasmAbi for ErrorWrapper<T>
where
	<T as tsify::Tsify>::JsType: wasm_bindgen::convert::FromWasmAbi,
{
	type Abi = <<T as tsify::Tsify>::JsType as wasm_bindgen::convert::FromWasmAbi>::Abi;

	unsafe fn from_abi(js: Self::Abi) -> Self {
		let value: JsValue = <T as tsify::Tsify>::JsType::from_abi(js).into();

		let deserializer = serde_wasm_bindgen::Deserializer::from(value);

		match serde_path_to_error::deserialize(deserializer) {
			Ok(inner) => Self { inner },
			Err(err) => {
				wasm_bindgen::throw_str(&format!("failed to deserialize ({}): {}", err.path(), err.inner()));
			}
		}
	}
}

impl<T: serde::de::DeserializeOwned> ErrorWrapper<T> {
	pub fn into_inner(self) -> T {
		self.inner
	}
}

#[derive(tsify::Tsify, Debug, serde::Deserialize)]
/// Settings to configure the player.
#[tsify(from_wasm_abi)]
pub struct PlayerSettings {
	/// The organization id to use for the player.
	#[tsify(type = "string")]
	#[serde(deserialize_with = "deserialize_ulid")]
	pub organization_id: Ulid,

	/// The server to use for the player.
	/// By default the upstream server is the Official Scuffle Video Edge.
	/// Defaults to https://edge.scuffle.video
	#[tsify(optional, type = "string")]
	#[serde(default)]
	pub server: Option<Url>,

	/// Enable low latency mode.
	/// Low latency mode allows for sub second latency, but can cause buffering
	/// issues on slower connections. Defaults to true.
	#[tsify(optional)]
	#[serde(default)]
	pub enable_low_latency: Option<bool>,

	/// Enable ABR.
	/// Adaptive Bitrate allows for the player to switch between different
	/// quality levels based on your connection. Defaults to true if the room
	/// supports it.
	#[tsify(optional)]
	#[serde(default)]
	pub enable_abr: Option<bool>,

	/// Enable DVR.
	/// DVR allows for rewinding the stream for the duration of the recording.
	/// Defaults to true if the room supports it.
	#[tsify(optional)]
	#[serde(default)]
	pub enable_dvr: Option<bool>,

	/// Auto switch to audio only when player is not visible.
	/// This is useful for mobile devices, where you want to save bandwidth when
	/// the player is not visible. If enabled and the video is not visible for
	/// `audio_only_switch_delay_ms` milliseconds, the player will switch to
	/// audio only. Defaults to true.
	#[tsify(optional)]
	#[serde(default)]
	pub auto_audio_only: Option<bool>,

	/// The number of milliseconds before auto-switching to audio only mode,
	/// when the player is not visible. Will do nothing if `auto_audio_only` is
	/// false. Defaults to 5000ms
	#[tsify(optional)]
	#[serde(default)]
	pub audio_only_switch_delay_ms: Option<f64>,

	/// This is used to debounce frequent seek events.
	/// Such as dragging the seek bar.
	/// The number of milliseconds before a seek event is considered to be a new
	/// seek. Defaults to 100ms
	#[tsify(optional)]
	#[serde(default)]
	pub seeked_debounce_threshold_ms: Option<f64>,

	/// The max time that the player can be in the waiting state before seeking
	/// to patch the buffer hole. Defaults to 500ms
	#[tsify(optional)]
	#[serde(default)]
	pub waiting_threshold_ms: Option<f64>,

	/// The player drive cooldown in milliseconds.
	/// This is used to prevent the player from driving too often.
	/// Defaults to 0ms (no cooldown)
	#[tsify(optional)]
	#[serde(default)]
	pub player_drive_cooldown_ms: Option<f64>,

	/// The low latency seek threshold in milliseconds before we switch to
	/// real-time mode. If you seek with the player to lower than this
	/// threshold, the player will switch to real-time mode.
	/// Defaults to 4000ms
	#[tsify(optional)]
	#[serde(default)]
	pub low_latency_realtime_threshold_ms: Option<f64>,

	/// The normal latency seek threshold in milliseconds before we switch to
	/// real-time mode. If you seek with the player to lower than this
	/// threshold, the player will switch back to real-time mode.
	/// Defaults to 10000ms
	#[tsify(optional)]
	#[serde(default)]
	pub normal_latency_realtime_threshold_ms: Option<f64>,

	/// The target buffer duration in milliseconds for low latency mode.
	/// Defaults to 1000ms
	#[tsify(optional)]
	#[serde(default)]
	pub low_latency_target_buffer_duration_ms: Option<f64>,

	/// The target buffer duration in milliseconds for normal latency mode.
	/// Defaults to 8000ms
	#[tsify(optional)]
	#[serde(default)]
	pub normal_latency_target_buffer_duration_ms: Option<f64>,

	/// The target buffer duration in milliseconds for Static (DVR or Recording)
	/// mode. Defaults to 20000ms
	#[tsify(optional)]
	#[serde(default)]
	pub static_target_buffer_duration_ms: Option<f64>,

	/// The number of milliseconds before switching playback rates.
	/// This is used to stop the player from switching rates too often.
	/// Defaults to 2000ms
	#[tsify(optional)]
	#[serde(default)]
	pub playback_rate_change_cooldown_ms: Option<f64>,

	/// The normal rate for playback.
	/// Defaults to 1.0 (normal speed)
	#[tsify(optional)]
	#[serde(default)]
	pub normal_playback_rate: Option<f64>,

	/// The fast forward rate for playback.
	/// Defaults to 1.03 (3% faster than normal speed)
	/// This is used to speed up playback when the player is behind.
	#[tsify(optional)]
	#[serde(default)]
	pub fast_playback_rate: Option<f64>,

	/// The slow down rate for playback.
	/// Defaults to 0.97 (3% slower than normal speed)
	/// This is used to slow down playback when the player is ahead.
	#[tsify(optional)]
	#[serde(default)]
	pub slow_playback_rate: Option<f64>,

	/// The number of milliseconds before switching quality levels.
	/// This is used to stop the player from switching quality levels too often.
	/// Defaults to 5000ms
	#[tsify(optional)]
	#[serde(default)]
	pub abr_switch_cooldown_ms: Option<f64>,

	/// The number of milliseconds before sampling bandwidth rate.
	/// This is used to stop the player from sampling bandwidth too often.
	/// Defaults to 1000ms
	#[tsify(optional)]
	#[serde(default)]
	pub bandwidth_calc_cooldown_ms: Option<f64>,

	/// The number of samples to use when calculating bandwidth.
	/// We take the lowest bandwidth sample from the last
	/// `bandwidth_samples_size` samples. Defaults to 5
	#[tsify(optional)]
	#[serde(default)]
	pub bandwidth_samples_size: Option<i32>,

	/// In some browsers the page will automatically pause the player when the
	/// player is not visible. This threshold is used to determine if the player
	/// was paused by the browser or by the user.
	#[tsify(optional)]
	#[serde(default)]
	pub visibility_pause_threshold_ms: Option<f64>,

	/// The logging level to use for the player.
	/// Defaults to "info"
	#[tsify(optional)]
	#[serde(default)]
	pub logging_level: Option<LoggingLevel>,

	/// This parameter is used to tweek how the ABR EWMA behaves.
	/// The value denotes how fast values fall out of the fast bucket when in
	/// realtime mode. Defaults to 3.0
	#[tsify(optional)]
	#[serde(default)]
	pub abr_fast_realtime_half_life: Option<f64>,

	/// This parameter is used to tweek how the ABR EWMA behaves.
	/// The value denotes how fast values fall out of the slow bucket when in
	/// realtime mode. Defaults to 6.0
	#[tsify(optional)]
	#[serde(default)]
	pub abr_slow_realtime_half_life: Option<f64>,

	/// This parameter is used to tweek how the ABR EWMA behaves.
	/// The value denotes how fast values fall out of the fast bucket when in
	/// DVR or Recording mode. Defaults to 3.0
	#[tsify(optional)]
	#[serde(default)]
	pub abr_fast_half_life: Option<f64>,

	/// This parameter is used to tweek how the ABR EWMA behaves.
	/// The value denotes how fast values fall out of the slow bucket when in
	/// DVR or Recording mode. Defaults to 6.0
	#[tsify(optional)]
	#[serde(default)]
	pub abr_slow_half_life: Option<f64>,

	/// This parameter is used to provide a default bandwidth estimate when
	/// using ABR. Measured in bits
	/// Defaults to 5000000 (5Mbps)
	#[tsify(optional)]
	#[serde(default)]
	pub abr_default_bandwidth: Option<f64>,

	/// The number of milliseconds before refreshing the session.
	/// This is used to keep the session alive. If the session is not refreshed,
	/// the player will stop. For example, if they are watching a recording,
	/// after sometime if the player does not make a request to the server, the
	/// session will expire. Which would result in any further requests to the
	/// server to fail. Defaults to 60000ms (1 minute)
	#[tsify(optional)]
	#[serde(default)]
	pub session_refresh_interval_ms: Option<f64>,
}

macro_rules! if_set {
    ($value:ident => $target:ident { $($i:ident),* }) => {
        {
            $(
                if let Some($i) = $value.$i {
                    $target.$i = $i;
                }
            )*
        }
    };
}

impl From<PlayerSettings> for PlayerSettingsParsed {
	fn from(value: PlayerSettings) -> Self {
		let mut target = Self {
			organization_id: value.organization_id,
			..Default::default()
		};

		if_set!(value => target {
			server,
			enable_low_latency,
			enable_abr,
			enable_dvr,
			auto_audio_only,
			audio_only_switch_delay_ms,
			seeked_debounce_threshold_ms,
			waiting_threshold_ms,
			player_drive_cooldown_ms,
			low_latency_realtime_threshold_ms,
			normal_latency_realtime_threshold_ms,
			low_latency_target_buffer_duration_ms,
			normal_latency_target_buffer_duration_ms,
			static_target_buffer_duration_ms,
			playback_rate_change_cooldown_ms,
			normal_playback_rate,
			fast_playback_rate,
			slow_playback_rate,
			abr_switch_cooldown_ms,
			bandwidth_calc_cooldown_ms,
			bandwidth_samples_size,
			visibility_pause_threshold_ms,
			logging_level,
			abr_fast_realtime_half_life,
			abr_slow_realtime_half_life,
			abr_fast_half_life,
			abr_slow_half_life,
			abr_default_bandwidth,
			session_refresh_interval_ms
		});

		target
	}
}

#[derive(Debug)]
pub struct PlayerSettingsParsed {
	pub organization_id: Ulid,
	pub server: Url,
	pub enable_low_latency: bool,
	pub enable_abr: bool,
	pub enable_dvr: bool,
	pub auto_audio_only: bool,
	pub audio_only_switch_delay_ms: f64,
	pub seeked_debounce_threshold_ms: f64,
	pub waiting_threshold_ms: f64,
	pub player_drive_cooldown_ms: f64,
	pub low_latency_realtime_threshold_ms: f64,
	pub normal_latency_realtime_threshold_ms: f64,
	pub low_latency_target_buffer_duration_ms: f64,
	pub normal_latency_target_buffer_duration_ms: f64,
	pub static_target_buffer_duration_ms: f64,
	pub playback_rate_change_cooldown_ms: f64,
	pub normal_playback_rate: f64,
	pub fast_playback_rate: f64,
	pub slow_playback_rate: f64,
	pub abr_switch_cooldown_ms: f64,
	pub bandwidth_calc_cooldown_ms: f64,
	pub bandwidth_samples_size: i32,
	pub visibility_pause_threshold_ms: f64,
	pub logging_level: LoggingLevel,
	pub abr_fast_realtime_half_life: f64,
	pub abr_slow_realtime_half_life: f64,
	pub abr_fast_half_life: f64,
	pub abr_slow_half_life: f64,
	pub abr_default_bandwidth: f64,
	pub session_refresh_interval_ms: f64,
}

impl Default for PlayerSettingsParsed {
	fn default() -> Self {
		Self {
			// The organization id is not serde(default) because it is required from the user.
			// This value will be overwritten.
			organization_id: Ulid::nil(),
			server: "https://edge.scuffle.video".parse().unwrap(),
			enable_low_latency: true,
			enable_abr: true,
			enable_dvr: true,
			auto_audio_only: true,
			audio_only_switch_delay_ms: 5000.0,
			seeked_debounce_threshold_ms: 100.0,
			waiting_threshold_ms: 500.0,
			player_drive_cooldown_ms: 100.0,
			low_latency_realtime_threshold_ms: 4000.0,
			normal_latency_realtime_threshold_ms: 10000.0,
			low_latency_target_buffer_duration_ms: 1000.0,
			normal_latency_target_buffer_duration_ms: 8000.0,
			static_target_buffer_duration_ms: 20000.0,
			playback_rate_change_cooldown_ms: 2000.0,
			normal_playback_rate: 1.0,
			fast_playback_rate: 1.03,
			slow_playback_rate: 0.97,
			abr_switch_cooldown_ms: 5000.0,
			bandwidth_calc_cooldown_ms: 1000.0,
			bandwidth_samples_size: 5,
			visibility_pause_threshold_ms: 100.0,
			logging_level: LoggingLevel::Info,
			abr_fast_half_life: 3.0,
			abr_slow_half_life: 9.0,
			abr_fast_realtime_half_life: 3.0,
			abr_slow_realtime_half_life: 9.0,
			abr_default_bandwidth: 5.0 * 1000.0 * 1000.0,
			session_refresh_interval_ms: 1000.0 * 60.0,
		}
	}
}

impl PlayerSettingsParsed {
	pub fn logging_level(&self) -> tracing::Level {
		match self.logging_level {
			LoggingLevel::Info => tracing::Level::INFO,
			LoggingLevel::Trace => tracing::Level::TRACE,
			LoggingLevel::Debug => tracing::Level::DEBUG,
			LoggingLevel::Warn => tracing::Level::WARN,
			LoggingLevel::Error => tracing::Level::ERROR,
		}
	}
}
