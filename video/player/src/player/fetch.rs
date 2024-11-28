use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use gloo_timers::future::TimeoutFuture;
use js_sys::ArrayBuffer;
use tokio::sync::{broadcast, mpsc};
use url::Url;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{
	PerformanceObserver, PerformanceObserverEntryList, PerformanceObserverInit, PerformanceResourceTiming, XmlHttpRequest,
	XmlHttpRequestResponseType,
};

use super::util::{now, register_events, Holder};

thread_local!(static PERFORMANCE_OBSERVER: PerformanceObserverHolder = PerformanceObserverHolder::new());

pub type FetchResult<T> = Result<T, FetchError>;

struct PerformanceObserverHolder {
	_obs: PerformanceObserver,
	_cb: Closure<dyn FnMut(PerformanceObserverEntryList)>,
	entries: Rc<RefCell<Vec<PerformanceResourceTiming>>>,
}

impl PerformanceObserverHolder {
	fn new() -> Self {
		let entries = Rc::new(RefCell::new(Vec::new()));

		tracing::trace!("creating performance observer");

		let cb = {
			let entries = entries.clone();
			Closure::<dyn FnMut(_)>::new(move |items: PerformanceObserverEntryList| {
				let mut entries = entries.borrow_mut();
				entries.extend(
					items
						.get_entries()
						.iter()
						.map(|i| i.unchecked_into::<PerformanceResourceTiming>()),
				);

				let size = entries.len();
				if size > 250 {
					entries.drain(0..size - 250);
				}
			})
		};

		let obs = PerformanceObserver::new(cb.as_ref().unchecked_ref()).unwrap();

		obs.observe(&PerformanceObserverInit::new(&js_sys::Array::of1(&"resource".into())));

		Self {
			_obs: obs,
			entries,
			_cb: cb,
		}
	}

	fn get_entries_by_name(&self, name: &str) -> Vec<PerformanceResourceTiming> {
		let entries = self.entries.borrow();
		let entries = entries.iter().filter(|e| e.name() == name).cloned().collect();

		entries
	}
}

pub struct FetchRequest {
	url: Url,
	headers: HashMap<String, String>,
	method: String,
}

pub struct InflightRequest {
	url: Url,
	xhr: Holder<XmlHttpRequest, FetchEvent>,
	size: u32,
	start_time: f64,

	start_time_realitive: f64,
	ttfb_time_relative: f64,
	end_time_relative: f64,
}

#[derive(Debug, Default, Clone)]
pub struct Metrics {
	pub ttfb: f64,
	pub relative_ttfb: f64,
	pub total_duration: f64,
	pub size: u32,
	pub file_duration: f64,
}

enum FetchEvent {
	Headers(f64),
	LoadEnd(f64),
}

impl FetchRequest {
	pub fn new(method: &str, url: Url) -> Self {
		Self {
			url,
			headers: HashMap::new(),
			method: method.to_string(),
		}
	}

	pub fn start(&self, wakeup: broadcast::Sender<()>) -> FetchResult<InflightRequest> {
		// Make sure the performance observer is initialized
		PERFORMANCE_OBSERVER.with(|_| {});

		let req = XmlHttpRequest::new().map_err(FetchError::JsValue)?;

		req.set_response_type(XmlHttpRequestResponseType::Arraybuffer);

		req.open(&self.method, self.url.as_str()).map_err(FetchError::JsValue)?;

		for (key, value) in &self.headers {
			req.set_request_header(key, value).map_err(FetchError::JsValue)?;
		}

		req.send().map_err(FetchError::JsValue)?;

		// We need a large enough buffer to hold all events that can be fired
		// This is becasue we might read the events until the request is done
		let (tx, rx) = mpsc::channel(2);

		let cb = register_events!(req, {
			"loadend" => {
				let tx = tx.clone();
				move |e: web_sys::Event| {
					wakeup.send(()).ok();
					tx.try_send(FetchEvent::LoadEnd(e.time_stamp())).ok();
				}
			},
			"readystatechange" => {
				let req = req.clone();
				move |e: web_sys::Event| {
					if req.ready_state() == 2 {
						tx.try_send(FetchEvent::Headers(e.time_stamp())).ok();
					}
				}
			}
		});

		let xhr = Holder::new(req, rx, cb);

		Ok(InflightRequest {
			url: self.url.clone(),
			xhr,
			size: 0,
			end_time_relative: -1.0,
			ttfb_time_relative: -1.0,
			start_time: js_sys::Date::now(),
			start_time_realitive: now(),
		})
	}

	pub fn url(&self) -> &Url {
		&self.url
	}
}

#[derive(Debug)]
pub enum FetchError {
	JsValue(JsValue),
	StatusCode(u16, Vec<u8>),
	Json(serde_json::Error),
	EmptyResponse,
	Aborted,
	InvalidResponse,
}

impl InflightRequest {
	pub async fn wait_result(&mut self) -> FetchResult<Vec<u8>> {
		while self.end_time_relative == -1.0 {
			match self.xhr.events().recv().await {
				Some(FetchEvent::LoadEnd(end)) => {
					self.end_time_relative = end;
				}
				Some(FetchEvent::Headers(ttfb)) => {
					self.ttfb_time_relative = ttfb;
				}
				None => {
					return Err(FetchError::Aborted);
				}
			}
		}

		while !self.is_done() {
			TimeoutFuture::new(0).await;
		}

		let result = self.result()?;

		result.ok_or(FetchError::EmptyResponse)
	}

	pub fn is_done(&self) -> bool {
		self.xhr.ready_state() == XmlHttpRequest::DONE && self.metrics(0.0).is_some()
	}

	pub fn metrics(&self, file_duration: f64) -> Option<Metrics> {
		let entity = PERFORMANCE_OBSERVER.with(|p| p.get_entries_by_name(self.url.as_str()));

		let entity = entity.first()?;

		if entity.response_start() == 0.0 {
			// Most likely the server does not add the CORS headers
			Some(Metrics {
				size: self.size,
				ttfb: 0.0,
				relative_ttfb: self.ttfb_time_relative - self.start_time_realitive,
				total_duration: self.end_time_relative - self.start_time_realitive,
				file_duration,
			})
		} else {
			Some(Metrics {
				ttfb: entity.response_start() - entity.fetch_start(),
				size: entity.transfer_size() as u32,
				relative_ttfb: self.ttfb_time_relative - self.start_time_realitive,
				total_duration: self.end_time_relative - self.start_time_realitive,
				file_duration,
			})
		}
	}

	pub fn result(&mut self) -> FetchResult<Option<Vec<u8>>> {
		if !self.is_done() {
			return Ok(None);
		}

		while let Ok(resp) = self.xhr.events().try_recv() {
			match resp {
				FetchEvent::Headers(ttfb) => {
					self.ttfb_time_relative = ttfb;
				}
				FetchEvent::LoadEnd(end) => {
					self.end_time_relative = end;
				}
			}
		}

		if self.ttfb_time_relative == -1.0 {
			self.ttfb_time_relative = now();
		}

		if self.end_time_relative == -1.0 {
			self.end_time_relative = now();
		}

		let resp = self.xhr.response().map_err(FetchError::JsValue)?;
		if resp.is_null() {
			return Err(FetchError::EmptyResponse);
		}

		let Some(buf) = resp.dyn_ref::<ArrayBuffer>() else {
			return Err(FetchError::InvalidResponse);
		};

		let data = js_sys::Uint8Array::new(buf).to_vec();

		let status = self.xhr.status().map_err(FetchError::JsValue)?;

		if let Ok(Some(date)) = self.xhr.get_response_header("Date") {
			let js_date = js_sys::Date::new(&JsValue::from(date.as_str())).get_time();
			if !js_date.is_normal() || self.start_time < js_date + 5000.0 {
				self.size = data.len() as u32;
			} else {
				self.size = 0;
			}
		} else {
			self.size = data.len() as u32;
		}

		if !(200..399).contains(&status) {
			return Err(FetchError::StatusCode(status, data));
		}

		Ok(Some(data))
	}

	pub fn abort(&self) {
		if self.xhr.ready_state() != XmlHttpRequest::DONE {
			self.xhr.abort().ok();
		}
	}
}

impl Drop for InflightRequest {
	fn drop(&mut self) {
		self.abort();
	}
}
