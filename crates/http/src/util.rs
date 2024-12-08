#![cfg_attr(not(any(feature = "http1", feature = "http2", feature = "http3")), allow(dead_code))]

pub struct AbortOnDrop<T>(Option<tokio::task::JoinHandle<T>>);

impl<T> AbortOnDrop<T> {
	pub fn new(handle: tokio::task::JoinHandle<T>) -> Self {
		Self(Some(handle))
	}

	pub fn disarm(mut self) -> tokio::task::JoinHandle<T> {
		self.0.take().expect("disarmed twice")
	}
}

impl<T> Drop for AbortOnDrop<T> {
	fn drop(&mut self) {
		if let Some(handle) = self.0.take() {
			handle.abort();
		}
	}
}

use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

#[cfg_attr(not(any(feature = "_tcp", feature = "_quic")), allow(dead_code))]
pub struct TimeoutTracker {
	timeout: tokio::time::Duration,
	notify: tokio::sync::Notify,
	requests_inflight: AtomicUsize,
}

pub struct TimeoutTrackerDropGuard(Arc<TimeoutTracker>);

impl Drop for TimeoutTrackerDropGuard {
	fn drop(&mut self) {
		self.0.decr_inflight();
	}
}

#[cfg_attr(not(any(feature = "_tcp", feature = "_quic")), allow(dead_code))]
impl TimeoutTracker {
	pub fn new(timeout: tokio::time::Duration) -> Self {
		Self {
			timeout,
			notify: tokio::sync::Notify::new(),
			requests_inflight: AtomicUsize::new(0),
		}
	}

	pub fn new_guard(self: &Arc<Self>) -> TimeoutTrackerDropGuard {
		self.incr_inflight();
		TimeoutTrackerDropGuard(self.clone())
	}

	fn incr_inflight(&self) {
		self.requests_inflight.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
	}

	fn decr_inflight(&self) {
		if self.requests_inflight.fetch_sub(1, std::sync::atomic::Ordering::Relaxed) == 1 {
			self.notify.notify_one();
		}
	}

	pub async fn wait(&self) {
		loop {
			match futures::future::select(
				std::pin::pin!(tokio::time::sleep(self.timeout)),
				std::pin::pin!(self.notify.notified()),
			)
			.await
			{
				futures::future::Either::Left(_)
					if self.requests_inflight.load(std::sync::atomic::Ordering::Relaxed) == 0 =>
				{
					break;
				}
				_ => {}
			}
		}
	}
}
