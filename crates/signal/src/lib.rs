#![doc = include_str!("../README.md")]

use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::signal::unix::{Signal, SignalKind};

#[cfg(feature = "bootstrap")]
mod bootstrap;

#[cfg(feature = "bootstrap")]
pub use bootstrap::{SignalConfig, SignalSvc};

/// A handler for listening to multiple Unix signals, and providing a future for
/// receiving them.
///
/// This is useful for applications that need to listen for multiple signals,
/// and want to react to them in a non-blocking way. Typically you would need to
/// use a tokio::select{} to listen for multiple signals, but this provides a
/// more ergonomic interface for doing so.
///
/// After a signal is received you can poll the handler again to wait for
/// another signal. Dropping the handle will cancel the signal subscription
#[derive(Debug)]
#[must_use = "signal handlers must be used to wait for signals"]
pub struct SignalHandler {
	signals: Vec<(SignalKind, Signal)>,
}

impl Default for SignalHandler {
	fn default() -> Self {
		Self::new()
	}
}

impl SignalHandler {
	/// Create a new `SignalHandler` with no signals.
	pub const fn new() -> Self {
		Self { signals: Vec::new() }
	}

	/// Create a new `SignalHandler` with the given signals.
	pub fn with_signals(signals: impl IntoIterator<Item = SignalKind>) -> Self {
		let mut handler = Self::new();

		for signal in signals {
			handler = handler.with_signal(signal);
		}

		handler
	}

	/// Add a signal to the handler.
	///
	/// If the signal is already in the handler, it will not be added again.
	pub fn with_signal(mut self, kind: SignalKind) -> Self {
		if self.signals.iter().any(|(k, _)| k == &kind) {
			return self;
		}

		let signal = tokio::signal::unix::signal(kind).expect("failed to create signal");

		self.signals.push((kind, signal));

		self
	}

	/// Add a signal to the handler.
	///
	/// If the signal is already in the handler, it will not be added again.
	pub fn add_signal(&mut self, kind: SignalKind) -> &mut Self {
		if self.signals.iter().any(|(k, _)| k == &kind) {
			return self;
		}

		let signal = tokio::signal::unix::signal(kind).expect("failed to create signal");

		self.signals.push((kind, signal));

		self
	}

	/// Wait for a signal to be received.
	/// This is equivilant to calling (&mut handler).await, but is more
	/// ergonomic if you want to not take ownership of the handler.
	pub async fn recv(&mut self) -> SignalKind {
		self.await
	}

	/// Poll for a signal to be received.
	/// Does not require Pinning the handler.
	pub fn poll_recv(&mut self, cx: &mut Context<'_>) -> Poll<SignalKind> {
		for (kind, signal) in self.signals.iter_mut() {
			if signal.poll_recv(cx).is_ready() {
				return Poll::Ready(*kind);
			}
		}

		Poll::Pending
	}
}

impl std::future::Future for SignalHandler {
	type Output = SignalKind;

	fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		self.poll_recv(cx)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn raise_signal(kind: SignalKind) {
		// Safety: This is a test, and we control the process.
		unsafe {
			libc::raise(kind.as_raw_value());
		}
	}

	#[tokio::test]
	async fn test_signal_handler() {
		let mut handler = SignalHandler::new()
			.with_signal(SignalKind::user_defined1())
			.with_signal(SignalKind::user_defined2());

		raise_signal(SignalKind::user_defined1());

		let recv = tokio::time::timeout(tokio::time::Duration::from_millis(5), &mut handler)
			.await
			.unwrap();

		assert_eq!(recv, SignalKind::user_defined1(), "expected SIGUSR1");

		// We already received the signal, so polling again should return Poll::Pending
		let recv = tokio::time::timeout(tokio::time::Duration::from_millis(5), &mut handler).await;

		assert!(recv.is_err(), "expected timeout");

		raise_signal(SignalKind::user_defined2());

		// We should be able to receive the signal again
		let recv = tokio::time::timeout(tokio::time::Duration::from_millis(5), &mut handler)
			.await
			.unwrap();

		assert_eq!(recv, SignalKind::user_defined2(), "expected SIGUSR2");
	}
}
