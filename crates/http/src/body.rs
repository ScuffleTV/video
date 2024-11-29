use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::{Buf, Bytes};
use http_body::Frame;

#[cfg(feature = "_quic")]
use crate::backend::quic::QuicIncomingBody;

pub struct IncomingBody {
	inner: IncomingBodyInner,
}

impl IncomingBody {
	#[cfg_attr(not(any(feature = "_quic", feature = "_tcp")), allow(dead_code))]
	pub(crate) fn new(inner: impl Into<IncomingBodyInner>) -> Self {
		Self { inner: inner.into() }
	}

	#[cfg_attr(not(any(feature = "_quic", feature = "_tcp")), allow(dead_code))]
	pub(crate) fn empty() -> Self {
		Self {
			inner: IncomingBodyInner::Empty,
		}
	}
}

impl std::fmt::Debug for IncomingBody {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("IncomingBody").finish()
	}
}

#[derive(derive_more::From)]
pub(crate) enum IncomingBodyInner {
	#[cfg(feature = "_tcp")]
	Tcp(#[from] hyper::body::Incoming),
	#[cfg(feature = "_quic")]
	Quic(#[from] QuicIncomingBody),
	#[cfg_attr(not(any(feature = "_quic", feature = "_tcp")), allow(dead_code))]
	Empty,
}

impl http_body::Body for IncomingBody {
	type Data = Bytes;
	type Error = crate::Error;

	fn poll_frame(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
		#[cfg(not(any(feature = "_quic", feature = "_tcp")))]
		let _ = cx;

		match &mut self.inner {
			#[cfg(feature = "_tcp")]
			IncomingBodyInner::Tcp(body) => Pin::new(body).poll_frame(cx).map_err(Into::into),
			#[cfg(feature = "_quic")]
			IncomingBodyInner::Quic(body) => Pin::new(body).poll_frame(cx).map_err(Into::into),
			IncomingBodyInner::Empty => Poll::Ready(None),
		}
	}

	fn is_end_stream(&self) -> bool {
		match &self.inner {
			#[cfg(feature = "_tcp")]
			IncomingBodyInner::Tcp(body) => body.is_end_stream(),
			#[cfg(feature = "_quic")]
			IncomingBodyInner::Quic(body) => body.is_end_stream(),
			IncomingBodyInner::Empty => true,
		}
	}

	fn size_hint(&self) -> http_body::SizeHint {
		match &self.inner {
			#[cfg(feature = "_tcp")]
			IncomingBodyInner::Tcp(body) => body.size_hint(),
			#[cfg(feature = "_quic")]
			IncomingBodyInner::Quic(body) => body.size_hint(),
			IncomingBodyInner::Empty => http_body::SizeHint::with_exact(0),
		}
	}
}

pin_project_lite::pin_project! {
	pub struct TrackedBody<B, T> {
		#[pin]
		body: B,
		tracker: T,
	}
}

impl<B, T> TrackedBody<B, T> {
	pub fn new(body: B, tracker: T) -> Self {
		Self { body, tracker }
	}
}

pub trait Tracker: Send + Sync + 'static {
	type Error: Into<crate::Error> + Send + Sync + 'static;

	fn on_data(&self, size: usize) -> Result<(), Self::Error> {
		let _ = size;
		Ok(())
	}

	fn on_close(&self) {}
}

impl<B, T> http_body::Body for TrackedBody<B, T>
where
	B: http_body::Body,
	B::Error: Into<crate::Error> + Send + Sync + 'static,
	T: Tracker,
{
	type Data = B::Data;
	type Error = crate::Error;

	fn poll_frame(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
		let this = self.project();
		match this.body.poll_frame(cx) {
			Poll::Pending => Poll::Pending,
			Poll::Ready(frame) => {
				if let Some(Ok(frame)) = &frame {
					if let Some(data) = frame.data_ref() {
						if let Err(err) = this.tracker.on_data(data.remaining()) {
							return Poll::Ready(Some(Err(err.into())));
						}
					}
				}

				Poll::Ready(frame.transpose().map_err(Into::into).transpose())
			}
		}
	}

	fn is_end_stream(&self) -> bool {
		self.body.is_end_stream()
	}

	fn size_hint(&self) -> http_body::SizeHint {
		self.body.size_hint()
	}
}

#[cfg_attr(not(any(feature = "_quic", feature = "_tcp")), allow(dead_code))]
pub(crate) fn has_body(method: &http::Method) -> bool {
	!matches!(
		method,
		&http::Method::GET | &http::Method::HEAD | &http::Method::OPTIONS | &http::Method::CONNECT | &http::Method::TRACE
	)
}
