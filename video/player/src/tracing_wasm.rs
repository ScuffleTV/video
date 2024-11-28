// Taken from https://github.com/old-storyai/tracing-wasm in order to implement different logging levels.

use core::fmt::{
	Write, {self},
};
use core::sync::atomic::AtomicUsize;
use std::sync::Arc;

use tracing::field::{Field, Visit};
use tracing::Subscriber;
use tracing_subscriber::layer::*;
use tracing_subscriber::registry::*;
use wasm_bindgen::JsValue;
use web_sys::{console, window};

fn mark(name: &str) {
	window().unwrap().performance().unwrap().mark(name).unwrap();
}

fn measure(name: String, start_mark: String) -> Result<(), JsValue> {
	window()
		.unwrap()
		.performance()
		.unwrap()
		.measure_with_start_mark(&name, &start_mark)
}

#[derive(Debug, PartialEq, Clone)]
pub struct WASMLayerConfig {
	report_logs_in_timings: bool,
	report_logs_in_console: bool,
	use_console_color: bool,
	max_level: tracing::Level,
}

impl WASMLayerConfig {
	pub fn new(max_level: tracing::Level) -> Self {
		WASMLayerConfig {
			max_level,
			..Default::default()
		}
	}
}

impl core::default::Default for WASMLayerConfig {
	fn default() -> Self {
		WASMLayerConfig {
			report_logs_in_timings: true,
			report_logs_in_console: true,
			use_console_color: true,
			max_level: tracing::Level::TRACE,
		}
	}
}

/// Implements [tracing_subscriber::layer::Layer] which uses [wasm_bindgen] for
/// marking and measuring with `window.performance`
#[derive(Clone)]
pub struct WASMLayer {
	last_event_id: Arc<AtomicUsize>,
	config: WASMLayerConfig,
}

impl WASMLayer {
	pub fn new(config: WASMLayerConfig) -> Self {
		WASMLayer {
			last_event_id: Arc::new(AtomicUsize::new(0)),
			config,
		}
	}
}

impl core::default::Default for WASMLayer {
	fn default() -> Self {
		WASMLayer::new(WASMLayerConfig::default())
	}
}

fn mark_name(id: &tracing::Id) -> String {
	format!("t{:x}", id.into_u64())
}

impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for WASMLayer {
	fn enabled(&self, metadata: &tracing::Metadata<'_>, _: Context<'_, S>) -> bool {
		let level = metadata.level();
		level <= &self.config.max_level
	}

	fn on_new_span(&self, attrs: &tracing::span::Attributes<'_>, id: &tracing::Id, ctx: Context<'_, S>) {
		let mut new_debug_record = StringRecorder::new();
		attrs.record(&mut new_debug_record);

		if let Some(span_ref) = ctx.span(id) {
			span_ref.extensions_mut().insert::<StringRecorder>(new_debug_record);
		}
	}

	/// doc: Notifies this layer that a span with the given Id recorded the
	/// given values.
	fn on_record(&self, id: &tracing::Id, values: &tracing::span::Record<'_>, ctx: Context<'_, S>) {
		if let Some(span_ref) = ctx.span(id) {
			if let Some(debug_record) = span_ref.extensions_mut().get_mut::<StringRecorder>() {
				values.record(debug_record);
			}
		}
	}

	// /// doc: Notifies this layer that a span with the ID span recorded that it
	// follows from the span with the ID follows. fn on_follows_from(&self, _span:
	// &tracing::Id, _follows: &tracing::Id, ctx: Context<'_, S>) {}
	/// doc: Notifies this layer that an event has occurred.
	fn on_event(&self, event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
		if self.config.report_logs_in_timings || self.config.report_logs_in_console {
			let mut recorder = StringRecorder::new();
			event.record(&mut recorder);
			let meta = event.metadata();
			let level = meta.level();
			if self.config.report_logs_in_console {
				let origin = meta
					.file()
					.and_then(|file| meta.line().map(|ln| format!("{}:{}", file, ln)))
					.unwrap_or_default();

				if self.config.use_console_color {
					let console_fn = match *level {
						tracing::Level::TRACE => console::debug_4,
						tracing::Level::DEBUG => console::debug_4,
						tracing::Level::INFO => console::info_4,
						tracing::Level::WARN => console::warn_4,
						tracing::Level::ERROR => console::error_4,
					};
					console_fn(
						&format!("%c{}%c {}%c{}", level, origin, recorder,).into(),
						&match *level {
							tracing::Level::TRACE => "color: dodgerblue; background: #444",
							tracing::Level::DEBUG => "color: lawngreen; background: #444",
							tracing::Level::INFO => "color: whitesmoke; background: #444",
							tracing::Level::WARN => "color: orange; background: #444",
							tracing::Level::ERROR => "color: red; background: #444",
						}
						.into(),
						&"color: gray; font-style: italic".into(),
						&"color: inherit".into(),
					);
				} else {
					let console_fn = match *level {
						tracing::Level::TRACE => console::debug_1,
						tracing::Level::DEBUG => console::debug_1,
						tracing::Level::INFO => console::info_1,
						tracing::Level::WARN => console::warn_1,
						tracing::Level::ERROR => console::error_1,
					};

					console_fn(&format!("{} {} {}", level, origin, recorder,).into());
				}
			}
			if self.config.report_logs_in_timings {
				let mark_name = format!(
					"c{:x}",
					self.last_event_id.fetch_add(1, core::sync::atomic::Ordering::Relaxed)
				);
				// mark and measure so you can see a little blip in the profile
				mark(&mark_name);
				let _ = measure(
					format!("{} {} {}", level, meta.module_path().unwrap_or("..."), recorder,),
					mark_name,
				);
			}
		}
	}

	/// doc: Notifies this layer that a span with the given ID was entered.
	fn on_enter(&self, id: &tracing::Id, _ctx: Context<'_, S>) {
		mark(&mark_name(id));
	}

	/// doc: Notifies this layer that the span with the given ID was exited.
	fn on_exit(&self, id: &tracing::Id, ctx: Context<'_, S>) {
		if let Some(span_ref) = ctx.span(id) {
			let meta = span_ref.metadata();
			if let Some(debug_record) = span_ref.extensions().get::<StringRecorder>() {
				let _ = measure(
					format!("\"{}\" {} {}", meta.name(), meta.module_path().unwrap_or("..."), debug_record,),
					mark_name(id),
				);
			} else {
				let _ = measure(
					format!("\"{}\" {}", meta.name(), meta.module_path().unwrap_or("..."),),
					mark_name(id),
				);
			}
		}
	}
	// /// doc: Notifies this layer that the span with the given ID has been closed.
	// /// We can dispose of any data for the span we might have here...
	// fn on_close(&self, _id: tracing::Id, ctx: Context<'_, S>) {}
	// /// doc: Notifies this layer that a span ID has been cloned, and that the
	// subscriber returned a different ID. /// I'm not sure if I need to do
	// something here... fn on_id_change(&self, _old: &tracing::Id, _new:
	// &tracing::Id, ctx: Context<'_, S>) {}
}

/// Set the global default with [tracing::subscriber::set_global_default]
pub fn set_as_global_default() {
	tracing::subscriber::set_global_default(registry(WASMLayerConfig::default())).expect("default global");
}

pub type LoggingInstance = Layered<WASMLayer, Registry>;

pub fn registry(config: WASMLayerConfig) -> LoggingInstance {
	Registry::default().with(WASMLayer::new(config))
}

pub fn set_default(config: WASMLayerConfig) -> tracing_core::dispatcher::DefaultGuard {
	tracing::subscriber::set_default(Registry::default().with(WASMLayer::new(config)))
}

struct StringRecorder {
	display: String,
	is_following_args: bool,
}
impl StringRecorder {
	fn new() -> Self {
		StringRecorder {
			display: String::new(),
			is_following_args: false,
		}
	}
}

impl Visit for StringRecorder {
	fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
		if field.name() == "message" {
			if !self.display.is_empty() {
				self.display = format!("{:?}\n{}", value, self.display)
			} else {
				self.display = format!("{:?}", value)
			}
		} else {
			if self.is_following_args {
				// following args
				writeln!(self.display).unwrap();
			} else {
				// first arg
				write!(self.display, " ").unwrap();
				self.is_following_args = true;
			}
			write!(self.display, "{} = {:?};", field.name(), value).unwrap();
		}
	}
}

impl core::fmt::Display for StringRecorder {
	fn fmt(&self, mut f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
		if !self.display.is_empty() {
			write!(&mut f, " {}", self.display)
		} else {
			Ok(())
		}
	}
}

impl core::default::Default for StringRecorder {
	fn default() -> Self {
		StringRecorder::new()
	}
}

macro_rules! scope {
	($inner:expr) => {
		let __ = crate::tracing_wasm::set_default(crate::tracing_wasm::WASMLayerConfig::new(
			$inner.interface_settings.player_settings.logging_level(),
		));
	};
}

pub(super) use scope;
