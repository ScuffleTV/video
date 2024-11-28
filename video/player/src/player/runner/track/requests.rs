use std::collections::VecDeque;

use tokio::sync::broadcast;

use crate::player::fetch::{FetchRequest, FetchResult, InflightRequest};
use crate::player::util::now;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RequestIndex {
	Init,
	Segment { idx: u32, start_time: f64, end_time: f64 },
	Part { idx: u32 },
}

pub struct TrackRequest {
	pub request: FetchRequest,
	pub index: RequestIndex,
	pub initial_fail_time: f64,
	pub start_at: f64,
	pub inflight: Option<InflightRequest>,
}

impl std::fmt::Debug for TrackRequest {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("TrackRequest")
			.field("url", &self.request.url())
			.field("inflight", &self.inflight.is_some())
			.finish()
	}
}

impl TrackRequest {
	pub fn new(req: FetchRequest, index: RequestIndex) -> Self {
		Self {
			request: req,
			index,
			inflight: None,
			start_at: 0.0,
			initial_fail_time: 0.0,
		}
	}

	pub fn start(&mut self, wakeup: &broadcast::Sender<()>) -> FetchResult<()> {
		if self.inflight.is_none() && self.start_at <= now() {
			self.inflight = Some(self.request.start(wakeup.clone())?);
		}

		Ok(())
	}

	pub fn new_with_start(req: FetchRequest, index: RequestIndex, wakeup: &broadcast::Sender<()>) -> FetchResult<Self> {
		let mut this = Self::new(req, index);
		this.start(wakeup)?;
		Ok(this)
	}
}

#[derive(Debug, Default)]
pub struct RequestQueue {
	requests: VecDeque<TrackRequest>,
}

impl RequestQueue {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn requeue(&mut self, mut req: TrackRequest) {
		let now = now();

		if req.initial_fail_time == 0.0 {
			req.initial_fail_time = now;
		}

		if now - req.initial_fail_time > 1000.0 {
			tracing::warn!("dropping old request due to previous failures");
			return;
		}

		req.inflight = None;
		req.start_at = now + 250.0;

		self.requests.push_front(req);
	}

	pub fn push(&mut self, req: TrackRequest) -> bool {
		if self.requests.iter().any(|r| r.request.url() == req.request.url()) {
			return false;
		}

		self.requests.push_back(req);
		true
	}

	pub fn start(&mut self, wakeup: &broadcast::Sender<()>) -> FetchResult<()> {
		if let Some(req) = self.requests.front_mut() {
			req.start(wakeup)?;
			if matches!(req.index, RequestIndex::Part { .. }) {
				for next in self.requests.iter_mut().skip(1).take(1) {
					if matches!(next.index, RequestIndex::Part { .. }) {
						next.start(wakeup)?;
					} else {
						break;
					}
				}
			}
		}

		Ok(())
	}

	pub fn clear(&mut self) {
		self.requests.clear();
	}

	pub fn done(&mut self) -> Option<TrackRequest> {
		if let Some(req) = self.requests.front() {
			if req.inflight.as_ref().map(|i| i.is_done()).unwrap_or_default() {
				return self.requests.pop_front();
			}
		}

		None
	}
}
