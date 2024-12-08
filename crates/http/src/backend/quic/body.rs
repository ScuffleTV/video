#![cfg_attr(not(feature = "quic-quinn"), allow(dead_code))]

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::{Buf, Bytes};
use h3::quic::{BidiStream, SendStream};
use h3::server::RequestStream;

#[derive(derive_more::From)]
pub(crate) enum QuicIncomingBody {
	#[cfg(feature = "quic-quinn")]
	Quinn(#[from] QuicIncomingBodyInner<h3_quinn::BidiStream<Bytes>>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
	Data(Option<u64>),
	Trailers,
	Done,
}

pub(crate) struct QuicIncomingBodyInner<B: BidiStream<Bytes>> {
	stream: RequestStream<B::RecvStream, Bytes>,
	state: State,
}

impl<B: BidiStream<Bytes>> QuicIncomingBodyInner<B> {
	#[cfg(feature = "quic-quinn")]
	pub(crate) fn new(stream: RequestStream<B::RecvStream, Bytes>, size_hint: Option<u64>) -> Self {
		Self {
			stream,
			state: State::Data(size_hint),
		}
	}
}

impl http_body::Body for QuicIncomingBody {
	type Data = Bytes;
	type Error = h3::Error;

	fn poll_frame(
		self: Pin<&mut Self>,
		cx: &mut Context<'_>,
	) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
		match self.get_mut() {
			#[cfg(feature = "quic-quinn")]
			QuicIncomingBody::Quinn(inner) => Pin::new(inner).poll_frame(cx),
			#[cfg(not(feature = "quic-quinn"))]
			_ => {
				let _ = cx;
				unreachable!("impossible to construct QuicIncomingBody with no transport")
			}
		}
	}

	fn size_hint(&self) -> http_body::SizeHint {
		match self {
			#[cfg(feature = "quic-quinn")]
			QuicIncomingBody::Quinn(inner) => Pin::new(inner).size_hint(),
			#[cfg(not(feature = "quic-quinn"))]
			_ => unreachable!("impossible to construct QuicIncomingBody with no transport"),
		}
	}

	fn is_end_stream(&self) -> bool {
		match self {
			#[cfg(feature = "quic-quinn")]
			QuicIncomingBody::Quinn(inner) => inner.is_end_stream(),
			#[cfg(not(feature = "quic-quinn"))]
			_ => unreachable!("impossible to construct QuicIncomingBody with no transport"),
		}
	}
}

impl<B: BidiStream<Bytes>> http_body::Body for QuicIncomingBodyInner<B> {
	type Data = Bytes;
	type Error = h3::Error;

	fn poll_frame(
		mut self: Pin<&mut Self>,
		cx: &mut Context<'_>,
	) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
		let QuicIncomingBodyInner { stream, state } = self.as_mut().get_mut();

		if *state == State::Done {
			return Poll::Ready(None);
		}

		if let State::Data(remaining) = state {
			match stream.poll_recv_data(cx) {
				Poll::Ready(Ok(Some(mut buf))) => {
					let buf_size = buf.remaining() as u64;

					if let Some(remaining) = remaining {
						if buf_size > *remaining {
							*state = State::Done;
							return Poll::Ready(Some(Err(h3::error::Code::H3_FRAME_UNEXPECTED.into())));
						}

						*remaining -= buf_size;
					}

					return Poll::Ready(Some(Ok(http_body::Frame::data(buf.copy_to_bytes(buf_size as usize)))));
				}
				Poll::Ready(Ok(None)) => {
					*state = State::Trailers;
				}
				Poll::Ready(Err(err)) => {
					*state = State::Done;
					return Poll::Ready(Some(Err(err)));
				}
				Poll::Pending => {
					return Poll::Pending;
				}
			}
		}

		// We poll the recv data again even though we already got the None
		// because we want to make sure there is not a frame after the trailers
		// This is a workaround because h3 does not allow us to poll the trailer
		// directly, so we need to make sure the future recv_trailers is going to be
		// ready after a single poll We avoid pinning to the heap.
		let resp = match stream.poll_recv_data(cx) {
			Poll::Ready(Ok(None)) => match std::pin::pin!(stream.recv_trailers()).poll(cx) {
				Poll::Ready(Ok(Some(trailers))) => Poll::Ready(Some(Ok(http_body::Frame::trailers(trailers)))),
				// We will only poll the recv_trailers once so if pending is returned we are done.
				Poll::Pending => {
					#[cfg(feature = "tracing")]
					tracing::warn!("recv_trailers is pending");
					Poll::Ready(None)
				}
				Poll::Ready(Ok(None)) => Poll::Ready(None),
				Poll::Ready(Err(err)) => Poll::Ready(Some(Err(err))),
			},
			// We are not expecting any data after the previous poll returned None
			Poll::Ready(Ok(Some(_))) => Poll::Ready(Some(Err(h3::error::Code::H3_FRAME_UNEXPECTED.into()))),
			Poll::Ready(Err(err)) => Poll::Ready(Some(Err(err))),
			Poll::Pending => return Poll::Pending,
		};

		*state = State::Done;

		resp
	}

	fn size_hint(&self) -> http_body::SizeHint {
		match self.state {
			State::Data(Some(remaining)) => http_body::SizeHint::with_exact(remaining),
			State::Data(None) => http_body::SizeHint::default(),
			State::Trailers | State::Done => http_body::SizeHint::with_exact(0),
		}
	}

	fn is_end_stream(&self) -> bool {
		match self.state {
			State::Data(Some(0)) | State::Trailers | State::Done => true,
			State::Data(_) => false,
		}
	}
}

pub(crate) async fn copy_body<E: Into<crate::Error>>(
	mut send: RequestStream<impl SendStream<Bytes>, Bytes>,
	body: impl http_body::Body<Error = E>,
) -> Result<(), crate::Error> {
	let mut body = std::pin::pin!(body);
	while let Some(frame) = std::future::poll_fn(|cx| body.as_mut().poll_frame(cx)).await {
		match frame {
			Ok(frame) => match frame.into_data().map_err(|f| f.into_trailers()) {
				Ok(mut data) => send.send_data(data.copy_to_bytes(data.remaining())).await?,
				Err(Ok(trailers)) => {
					send.send_trailers(trailers).await?;
					return Ok(());
				}
				Err(Err(_)) => continue,
			},
			Err(err) => return Err(err.into()),
		}
	}

	send.finish().await?;

	Ok(())
}
