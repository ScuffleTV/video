#[derive(Debug)]
pub struct Timings {
	pub current_player_time: f64,
	pub last_seeked: f64,
	pub last_drive: f64,
	pub last_time_update: f64,
	pub last_rate_change: f64,
	pub last_abr_switch: f64,
	pub last_session_refresh: f64,
	pub document_visible: Option<f64>,
	pub waiting: Option<f64>,
}

impl Default for Timings {
	fn default() -> Self {
		Self {
			current_player_time: 0.0,
			last_seeked: -1.0,
			last_drive: -1.0,
			last_time_update: -1.0,
			last_rate_change: -1.0,
			last_abr_switch: -1.0,
			last_session_refresh: -1.0,
			document_visible: None,
			waiting: None,
		}
	}
}

impl Timings {
	pub fn reset(&mut self) {
		*self = Self::default();
	}
}

#[derive(Debug, Default)]
pub struct Flags {
	pub is_stopped: bool,
	pub is_finished: bool,
}

impl Flags {
	pub fn reset(&mut self) {
		*self = Self::default();
	}
}
