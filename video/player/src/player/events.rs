use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use tsify::JsValueSerdeExt;
use wasm_bindgen::prelude::*;

use super::errors::EventError;
use super::inner::NextVariantAutoCause;

pub enum UserEvent {
	Error(EventError),
	LoadStart,
	Stopped,
	Started,
	ManifestLoaded,
	Variant(VariantEvent),
	LowLatency,
	Abr,
	Dvr,
	Visibility,
	Realtime,
	Destroyed,
	Finished,
}

#[derive(Debug, Clone, serde::Serialize, tsify::Tsify)]
/// The event emitted when the variant changes.
pub struct VariantEvent {
	/// The variant ID that was selected.
	pub variant_id: u32,

	/// If the variant was selected automatically, based on the ABR algorithm.
	pub automatic: Option<NextVariantAutoCause>,

	/// The previous variant ID.
	pub previous_variant_id: i32,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EventType {
	Error,
	LoadStart,
	ManifestLoaded,
	Variant,
	LowLatency,
	Abr,
	Dvr,
	Realtime,
	Destroyed,
	Stopped,
	Started,
	Finished,
	Visibility,
}

impl std::str::FromStr for EventType {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"error" => Ok(Self::Error),
			"loadstart" => Ok(Self::LoadStart),
			"manifestloaded" => Ok(Self::ManifestLoaded),
			"variant" => Ok(Self::Variant),
			"lowlatency" => Ok(Self::LowLatency),
			"abr" => Ok(Self::Abr),
			"dvr" => Ok(Self::Dvr),
			"realtime" => Ok(Self::Realtime),
			"destroyed" => Ok(Self::Destroyed),
			"stopped" => Ok(Self::Stopped),
			"started" => Ok(Self::Started),
			"finished" => Ok(Self::Finished),
			"visibility" => Ok(Self::Visibility),
			_ => Err(()),
		}
	}
}

impl UserEvent {
	const fn ty(&self) -> EventType {
		match self {
			Self::Error(_) => EventType::Error,
			Self::LoadStart => EventType::LoadStart,
			Self::ManifestLoaded => EventType::ManifestLoaded,
			Self::Variant(_) => EventType::Variant,
			Self::LowLatency => EventType::LowLatency,
			Self::Abr => EventType::Abr,
			Self::Dvr => EventType::Dvr,
			Self::Realtime => EventType::Realtime,
			Self::Destroyed => EventType::Destroyed,
			Self::Stopped => EventType::Stopped,
			Self::Started => EventType::Started,
			Self::Finished => EventType::Finished,
			Self::Visibility => EventType::Visibility,
		}
	}

	fn value(&self) -> Option<JsValue> {
		match self {
			Self::Error(error) => Some(serde_wasm_bindgen::to_value(error).unwrap()),
			Self::LoadStart => None,
			Self::ManifestLoaded => None,
			Self::LowLatency => None,
			Self::Abr => None,
			Self::Variant(change) => Some(JsValue::from_serde(&change).unwrap()),
			Self::Dvr => None,
			Self::Realtime => None,
			Self::Destroyed => None,
			Self::Stopped => None,
			Self::Started => None,
			Self::Finished => None,
			Self::Visibility => None,
		}
	}
}

impl From<EventError> for UserEvent {
	fn from(error: EventError) -> Self {
		Self::Error(error)
	}
}

#[derive(Clone)]
struct EventListener {
	f: js_sys::Function,
	used: Option<Rc<Cell<bool>>>,
}

pub struct EventManager {
	listeners: HashMap<EventType, Vec<EventListener>>,
	dirty: Rc<Cell<bool>>,
}

impl EventManager {
	pub fn new() -> Self {
		Self {
			listeners: HashMap::new(),
			dirty: Rc::new(Cell::new(false)),
		}
	}

	fn clean(&mut self) {
		if self.dirty.get() {
			for (_, listeners) in self.listeners.iter_mut() {
				listeners.retain(|x| if let Some(used) = x.used.as_ref() { !used.get() } else { true });
			}

			self.dirty.set(false);
		}
	}

	pub fn add_event_listener(&mut self, event: EventType, f: JsValue, once: bool) {
		self.clean();

		let listeners = self.listeners.entry(event).or_default();
		listeners.push(EventListener {
			f: f.unchecked_into(),
			used: if once { Some(Rc::new(Cell::new(false))) } else { None },
		});
	}

	pub fn remove_event_listener(&mut self, event: EventType, f: JsValue) {
		self.clean();

		if let Some(listeners) = self.listeners.get_mut(&event) {
			listeners.retain(|x| !JsValue::eq(&x.f, &f));
		}
	}

	#[must_use = "must be called to process events use disptach! macro"]
	pub fn emit(&mut self, event: impl Into<UserEvent>) -> impl FnOnce() + 'static {
		self.clean();

		let event = event.into();
		let dirty = self.dirty.clone();
		let listeners = self.listeners.clone();

		move || {
			let ty = event.ty();
			if let Some(listeners) = listeners.get(&ty) {
				let value = event.value();
				for listener in listeners {
					if let Some(used) = listener.used.as_ref() {
						if used.get() {
							continue;
						}
					}

					let func: &js_sys::Function = listener.f.unchecked_ref();
					if let Some(evt) = &value {
						if let Err(err) = func.call1(&JsValue::undefined(), evt) {
							tracing::error!("event target raised exception: {:?}", err);
						}
					} else if let Err(err) = func.call0(&JsValue::undefined()) {
						tracing::error!("event target raised exception: {:?}", err);
					}

					if let Some(used) = listener.used.as_ref() {
						used.set(true);
						dirty.set(true);
					}
				}
			}
		}
	}
}

macro_rules! dispatch {
	($x:expr) => {{
		let f = { $x };
		f()
	}};
}

pub(super) use dispatch;
