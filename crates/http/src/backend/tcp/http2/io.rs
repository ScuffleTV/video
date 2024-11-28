#![allow(dead_code)]

// TODO: Use BDP logic for bandwidth limiting & keep alive logic

use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use bytes::Bytes;
use spin::Mutex;

use super::ResultErrorExt;
use crate::error::{ErrorConfig, ErrorScope, ErrorSeverity};
use crate::Error;

#[derive(Clone)]
/// Shared state is shared between all streams on the same connection.
pub(crate) struct Http2SharedState {
	inner: Arc<Mutex<Http2SharedStateInner>>,
}

impl Http2SharedState {
	pub(crate) fn new(ping_pong: h2::PingPong) -> Self {
		Self {
			inner: Arc::new(Mutex::new(Http2SharedStateInner { ping_pong })),
		}
	}
}

struct Http2SharedStateInner {
	ping_pong: h2::PingPong,
}

#[derive(Clone, Copy)]
enum State {
	SizeKnown(u64),
	SizeUnknown,
	BodyComplete,
	TrailerComplete,
}

pin_project_lite::pin_project! {
	pub struct RecvStream {
		#[pin]
		io: h2::RecvStream,
		#[pin]
		state: State,
		shared: Http2SharedState,
	}
}

impl RecvStream {
	pub(crate) fn new(io: h2::RecvStream, size: Option<u64>, shared: Http2SharedState) -> Self {
		Self {
			io,
			state: size.map(State::SizeKnown).unwrap_or(State::SizeUnknown),
			shared,
		}
	}
}

impl http_body::Body for RecvStream {
	type Data = Bytes;
	type Error = crate::Error;

	fn poll_frame(
		self: Pin<&mut Self>,
		cx: &mut Context<'_>,
	) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
		let mut this = self.project();

		if matches!(*this.state, State::TrailerComplete) {
			return Poll::Ready(None);
		}

		if !matches!(*this.state, State::BodyComplete) {
			match this.io.poll_data(cx) {
				Poll::Pending => return Poll::Pending,
				Poll::Ready(Some(data)) => {
					let data = data.with_config(ErrorConfig {
						severity: ErrorSeverity::Unknown,
						scope: ErrorScope::Request,
						context: "http2 body data",
					})?;

					return Poll::Ready(Some(Ok(http_body::Frame::data(data))));
				}
				Poll::Ready(None) => {
					*this.state = State::BodyComplete;
				}
			}
		}

		match this.io.poll_trailers(cx) {
			Poll::Pending => Poll::Pending,
			Poll::Ready(Ok(None)) => {
				*this.state = State::TrailerComplete;
				Poll::Ready(None)
			}
			Poll::Ready(Ok(Some(trailers))) => {
				*this.state = State::TrailerComplete;
				Poll::Ready(Some(Ok(http_body::Frame::trailers(trailers))))
			}
			Poll::Ready(Err(e)) => {
				*this.state = State::BodyComplete;
				Poll::Ready(Some(Err(Error::from(e).with_config(ErrorConfig {
					severity: ErrorSeverity::Unknown,
					scope: ErrorScope::Request,
					context: "http2 body trailers",
				}))))
			}
		}
	}

	fn is_end_stream(&self) -> bool {
		self.io.is_end_stream() || matches!(self.state, State::TrailerComplete)
	}

	fn size_hint(&self) -> http_body::SizeHint {
		match self.state {
			State::SizeKnown(size) => http_body::SizeHint::with_exact(size),
			State::SizeUnknown => http_body::SizeHint::default(),
			State::BodyComplete | State::TrailerComplete => http_body::SizeHint::with_exact(0),
		}
	}
}
