use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use tokio::sync::broadcast;
use ulid::Ulid;
use url::Url;
use video_player_types::ThumbnailRange;

use super::api::ApiClient;
use super::bandwidth::Bandwidth;
use super::events::EventManager;
use super::settings::PlayerSettingsParsed;
use super::variant::Variant;

#[derive(Clone)]
pub struct PlayerInnerHolder(
	Rc<RefCell<PlayerInner>>,
	Rc<Cell<Option<&'static std::panic::Location<'static>>>>,
);

#[derive(Clone)]
pub struct PlayerInnerWeakHolder(
	Weak<RefCell<PlayerInner>>,
	Weak<Cell<Option<&'static std::panic::Location<'static>>>>,
);

impl PlayerInnerHolder {
	pub fn new(inner: PlayerInner) -> Self {
		Self(Rc::new(RefCell::new(inner)), Rc::new(Cell::new(None)))
	}

	#[track_caller]
	pub fn borrow(&self) -> std::cell::Ref<PlayerInner> {
		let borrow = self
			.0
			.try_borrow()
			.map_err(|err| {
				tracing::error!(
					"Failed to borrow player inner\nPrevious borrow location: {:?}\nNew Location: {:?}",
					self.1.get(),
					std::panic::Location::caller()
				);
				err
			})
			.expect("failed to borrow player inner");

		self.1.set(Some(std::panic::Location::caller()));

		borrow
	}

	#[track_caller]
	pub fn borrow_mut(&self) -> std::cell::RefMut<PlayerInner> {
		let borrow = self
			.0
			.try_borrow_mut()
			.map_err(|err| {
				tracing::error!(
					"Failed to borrow player inner\nPrevious borrow location: {:?}\nNew Location: {:?}",
					self.1.get(),
					std::panic::Location::caller()
				);
				err
			})
			.expect("failed to borrow player inner");

		self.1.set(Some(std::panic::Location::caller()));

		borrow
	}

	pub fn downgrade(&self) -> PlayerInnerWeakHolder {
		PlayerInnerWeakHolder(Rc::downgrade(&self.0), Rc::downgrade(&self.1))
	}
}

impl PlayerInnerWeakHolder {
	pub fn upgrade(&self) -> Option<PlayerInnerHolder> {
		let inner = self.0.upgrade()?;
		let location = self.1.upgrade()?;

		Some(PlayerInnerHolder(inner, location))
	}
}

#[derive(Debug)]
pub struct InterfaceSettings {
	pub target: Option<VideoTarget>,
	pub token: Option<String>,
	pub state: PlayerState,
	pub player_settings: PlayerSettingsParsed,
	pub next_variant_id: Option<NextVariant>,
	pub realtime_mode: bool,
	pub auto_seek: bool,
}

#[derive(Debug)]
pub struct RunnerSettings {
	pub current_variant_id: u32,
	pub dvr_supported: bool,
	pub realtime_supported: bool,
	pub variants: Vec<Variant>,
	pub thumbnail_prefix: Option<Url>,
	pub thumbnails: Vec<ThumbnailRange>,
	pub request_wakeup: broadcast::Sender<()>,
	pub visible: bool,
}

impl Default for RunnerSettings {
	fn default() -> Self {
		let (request_wakeup, _) = broadcast::channel(1);

		Self {
			current_variant_id: 0,
			dvr_supported: false,
			realtime_supported: false,
			variants: Vec::new(),
			thumbnail_prefix: None,
			thumbnails: Vec::new(),
			visible: true,
			request_wakeup,
		}
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
	Running,
	Stopped,
	Shutdown,
	Initialized,
}

#[derive(Debug, Clone, Copy, serde::Serialize, tsify::Tsify)]
#[serde(rename_all = "lowercase")]
pub enum NextVariantAutoCause {
	Bandwidth,
	Visibility,
}

#[derive(Debug, Clone, Copy)]
pub enum NextVariant {
	Switch(u32),
	Auto { id: u32, cause: NextVariantAutoCause },
	Force(u32),
}

impl NextVariant {
	pub fn variant_id(&self) -> u32 {
		match self {
			Self::Switch(id) | Self::Force(id) | Self::Auto { id, .. } => *id,
		}
	}

	pub fn is_force(&self) -> bool {
		matches!(self, Self::Force(_))
	}

	pub fn automatic(&self) -> Option<NextVariantAutoCause> {
		match self {
			Self::Auto { cause, .. } => Some(*cause),
			_ => None,
		}
	}
}

pub struct PlayerInner {
	pub video_element: web_sys::HtmlVideoElement,
	pub interface_settings: InterfaceSettings,
	pub client: ApiClient,
	pub runner_settings: RunnerSettings,
	pub events: EventManager,
	pub bandwidth: Bandwidth,
}

impl PlayerInner {
	pub fn use_dvr(&self, bypass_supported: bool) -> bool {
		self.interface_settings.player_settings.enable_dvr && (self.runner_settings.dvr_supported || bypass_supported)
	}

	pub fn set_realtime(&mut self, realtime: bool) -> Result<bool, &'static str> {
		if realtime && !self.runner_settings.realtime_supported {
			return Err("realtime is not supported, by the player, so it cannot be enabled");
		}

		if !realtime && !self.use_dvr(false) {
			return Err("dvr is not supported, by the player so realtime cannot be disabled");
		}

		if self.interface_settings.realtime_mode == realtime {
			return Ok(false);
		}

		self.interface_settings.realtime_mode = realtime;
		Ok(true)
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoTarget {
	Recording(Ulid),
	Room(Ulid),
}
