//! Provides the server side WebTransport session

use std::collections::HashMap;
use std::future::poll_fn;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use bytes::Buf;
use futures_util::future::{BoxFuture, Either};
use h3::error::ErrorLevel;
use h3::ext::{Datagram, Protocol};
use h3::frame::FrameStream;
use h3::proto::frame::Frame;
use h3::quic::{self, RecvStream as _, SendStream};
use h3::server::RequestStream;
use h3::stream::BufRecvStream;
use h3::webtransport::SessionId;
use http::{Method, Request};
use tokio::sync::{mpsc, oneshot};

use crate::stream::{BidiStream, RecvStream};

/// A struct used when upgrading a request to a webtransport session
pub(crate) struct WebTransportCanUpgrade<C, B>
where
	C: quic::Connection<B>,
	B: Buf,
{
	pub session_id: SessionId,
	pub webtransport_request_tx: mpsc::Sender<WebTransportRequest<C, B>>,
}

impl<C, B> Clone for WebTransportCanUpgrade<C, B>
where
	C: quic::Connection<B>,
	B: Buf,
{
	fn clone(&self) -> Self {
		Self {
			session_id: self.session_id,
			webtransport_request_tx: self.webtransport_request_tx.clone(),
		}
	}
}

type OnUpgrade<C, B> =
	Box<dyn FnOnce(RequestStream<C, B>) -> BoxFuture<'static, Result<(), h3::Error>> + Send + Sync + 'static>;

/// A struct used when upgrading a request to a webtransport session
pub struct WebTransportUpgradePending<C, B>
where
	C: quic::Connection<B>,
	B: Buf,
{
	#[allow(clippy::type_complexity)]
	pub(crate) complete_upgrade: Arc<Mutex<Option<OnUpgrade<C::BidiStream, B>>>>,
}

impl<C, B> WebTransportUpgradePending<C, B>
where
	C: quic::Connection<B>,
	B: Buf,
{
	/// Completes the upgrade to a WebTransport session
	#[allow(clippy::type_complexity)]
	pub fn upgrade(
		&self,
		stream: RequestStream<C::BidiStream, B>,
	) -> Result<BoxFuture<'static, Result<(), h3::Error>>, RequestStream<C::BidiStream, B>> {
		let Some(result) = Option::take(&mut *self.complete_upgrade.lock().unwrap()) else {
			return Err(stream);
		};

		Ok(result(stream))
	}
}

impl<C, B> Clone for WebTransportUpgradePending<C, B>
where
	C: quic::Connection<B>,
	B: Buf,
{
	fn clone(&self) -> Self {
		Self {
			complete_upgrade: self.complete_upgrade.clone(),
		}
	}
}

struct WebTransportSession<C, B>
where
	C: quic::Connection<B>,
	B: Buf,
{
	bidi: mpsc::Sender<BidiStream<C::BidiStream, B>>,
	uni: mpsc::Sender<RecvStream<C::RecvStream, B>>,
	datagram: mpsc::Sender<B>,
}

pub(crate) enum WebTransportRequest<C, B>
where
	C: quic::Connection<B>,
	B: Buf,
{
	Upgrade {
		session_id: SessionId,
		bidi_request: mpsc::Sender<BidiStream<C::BidiStream, B>>,
		uni_request: mpsc::Sender<RecvStream<C::RecvStream, B>>,
		datagram_request: mpsc::Sender<B>,
		response: oneshot::Sender<(C::OpenStreams, mpsc::UnboundedSender<SessionId>)>,
	},
	SendDatagram {
		session_id: SessionId,
		datagram: B,
		resp: oneshot::Sender<Result<(), h3::Error>>,
	},
}

/// A WebTransport server that allows incoming requests to be upgraded to
/// `WebTransportSessions`
///
/// The [`WebTransportServer`] struct manages a connection from the side of the
/// HTTP/3 server
///
/// Create a new Instance with [`WebTransportServer::new()`].
/// Accept incoming requests with [`WebTransportServer::accept()`].
/// And shutdown a connection with [`WebTransportServer::shutdown()`].
pub struct Connection<C, B>
where
	C: quic::Connection<B>,
	B: Buf,
{
	pub(crate) incoming: Incoming<C, B>,
	pub(crate) driver: ConnectionDriver<C, B>,
}

/// The driver for the WebTransport connection
pub struct ConnectionDriver<C, B>
where
	C: quic::Connection<B>,
	B: Buf,
{
	webtransport_session_map: HashMap<SessionId, WebTransportSession<C, B>>,
	#[allow(clippy::type_complexity)]
	request_sender: mpsc::Sender<(Request<()>, RequestStream<C::BidiStream, B>)>,
	webtransport_request_rx: mpsc::Receiver<WebTransportRequest<C, B>>,
	webtransport_request_tx: mpsc::Sender<WebTransportRequest<C, B>>,
	session_close_rx: mpsc::UnboundedReceiver<SessionId>,
	session_close_tx: mpsc::UnboundedSender<SessionId>,
	inner: h3::server::Connection<C, B>,
}

impl<C, B, E, E2> ConnectionDriver<C, B>
where
	C: quic::Connection<B> + quic::SendDatagramExt<B, Error = E> + quic::RecvDatagramExt<Buf = B, Error = E2> + 'static,
	B: Buf + 'static + Send + Sync,
	C::AcceptError: Send + Sync,
	C::BidiStream: Send + Sync,
	C::RecvStream: Send + Sync,
	C::OpenStreams: Send + Sync,
	E: Into<h3::Error>,
	E2: Into<h3::Error>,
{
	/// Drives the server, accepting requests from the underlying HTTP/3
	/// connection, and forwarding datagrams to the webtransport sessions
	pub async fn drive(&mut self) -> Result<(), h3::Error> {
		enum Winner<R, U, D, W> {
			Request(R),
			Uni(U),
			Datagram(D),
			WebTransport(W),
			Close(SessionId),
		}

		// Polls the underlying HTTP/3 connection for incoming requests
		// Yields a winner of either a request, a uni-stream, or a datagram (if enabled)
		let poll_inner = |this: &mut Self, cx: &mut Context<'_>| {
			match this.session_close_rx.poll_recv(cx) {
				Poll::Ready(Some(session_id)) => return Poll::Ready(Some(Ok(Winner::Close(session_id)))),
				Poll::Ready(None) => {}
				Poll::Pending => {}
			}

			match this.webtransport_request_rx.poll_recv(cx) {
				Poll::Ready(Some(r)) => return Poll::Ready(Some(Ok(Winner::WebTransport(r)))),
				Poll::Ready(None) => {}
				Poll::Pending => {}
			}

			match this.inner.poll_accept_request(cx) {
				Poll::Ready(Ok(None)) => return Poll::Ready(None),
				Poll::Ready(Ok(Some(r))) => return Poll::Ready(Some(Ok(Winner::Request(r)))),
				Poll::Ready(Err(err)) => return Poll::Ready(Some(Err(err))),
				Poll::Pending => {}
			}

			match this.inner.inner.poll_accept_recv(cx) {
				Ok(()) => {}
				Err(err) => return Poll::Ready(Some(Err(err))),
			}

			let streams = this.inner.inner.accepted_streams_mut();
			if let Some((id, stream)) = streams.wt_uni_streams.pop() {
				return Poll::Ready(Some(Ok(Winner::Uni((id, RecvStream::new(stream))))));
			}

			match this.inner.inner.conn.poll_accept_datagram(cx) {
				Poll::Ready(Ok(Some(r))) => match Datagram::decode(r) {
					Ok(d) => return Poll::Ready(Some(Ok(Winner::Datagram(d)))),
					Err(err) => return Poll::Ready(Some(Err(err))),
				},
				Poll::Ready(Ok(None)) => return Poll::Ready(None),
				Poll::Ready(Err(err)) => return Poll::Ready(Some(Err(err.into()))),
				Poll::Pending => {}
			}

			Poll::Pending
		};

		loop {
			let Some(winner) = poll_fn(|cx| poll_inner(self, cx)).await else {
				return Ok(());
			};

			let winner = match winner {
				Ok(w) => w,
				Err(err) => {
					match err.kind() {
						h3::error::Kind::Closed => return Ok(()),
						h3::error::Kind::Application {
							code,
							reason,
							level: ErrorLevel::ConnectionError,
							..
						} => {
							return Err(self
								.inner
								.close(code, reason.unwrap_or_else(|| String::into_boxed_str(String::from("")))))
						}
						_ => return Err(err),
					};
				}
			};

			let mut stream = match winner {
				Winner::Request(s) => FrameStream::new(BufRecvStream::new(s)),
				Winner::Uni((session_id, mut stream)) => {
					if let Some(webtransport) = self.webtransport_session_map.get_mut(&session_id) {
						match webtransport.uni.send(stream).await {
							Ok(_) => continue,
							Err(err) => {
								stream = err.0;
							}
						}
					}

					// We reject the stream because it is not a for a valid webtransport session
					stream.stop_sending(h3::error::Code::H3_REQUEST_REJECTED.value());
					continue;
				}
				Winner::Datagram(d) => {
					if let Some(webtransport) = self.webtransport_session_map.get_mut(&d.stream_id().into()) {
						// We dont care about datagram drops because they do not have any state.
						webtransport.datagram.send(d.into_payload()).await.ok();
					}
					continue;
				}
				Winner::WebTransport(WebTransportRequest::SendDatagram {
					session_id,
					datagram,
					resp,
				}) => {
					resp.send(self.inner.send_datagram(session_id.into(), datagram)).ok();
					continue;
				}
				Winner::WebTransport(WebTransportRequest::Upgrade {
					session_id,
					bidi_request,
					uni_request,
					datagram_request,
					response,
				}) => {
					if response
						.send((self.inner.inner.conn.opener(), self.session_close_tx.clone()))
						.is_ok()
					{
						self.webtransport_session_map.insert(
							session_id,
							WebTransportSession {
								bidi: bidi_request,
								uni: uni_request,
								datagram: datagram_request,
							},
						);
					}
					continue;
				}
				Winner::Close(session_id) => {
					self.webtransport_session_map.remove(&session_id);
					continue;
				}
			};

			// Read the first frame.
			//
			// This will determine if it is a webtransport bi-stream or a request stream
			let frame = poll_fn(|cx| stream.poll_next(cx)).await;

			match frame {
				Ok(None) => return Ok(()),
				Ok(Some(Frame::WebTransportStream(session_id))) => {
					let mut stream = BidiStream::new(stream.into_inner());
					if let Some(session) = self.webtransport_session_map.get_mut(&session_id) {
						match session.bidi.send(stream).await {
							Ok(_) => continue,
							Err(err) => {
								stream = err.0;
							}
						}
					}

					// We reject the stream because it is not a for a valid webtransport session
					stream.stop_sending(h3::error::Code::H3_REQUEST_REJECTED.value());
					stream.reset(h3::error::Code::H3_REQUEST_REJECTED.value());
					continue;
				}
				// Make the underlying HTTP/3 connection handle the rest
				frame => {
					let Some(req) = self.inner.accept_with_frame(stream, frame)? else {
						return Ok(());
					};

					let (mut req, resp) = req.resolve().await?;

					if validate_wt_connect(&req) {
						req.extensions_mut().insert(WebTransportCanUpgrade {
							session_id: resp.id().into(),
							webtransport_request_tx: self.webtransport_request_tx.clone(),
						});
					}

					if self.request_sender.send((req, resp)).await.is_err() {
						return Err(self
							.inner
							.close(h3::error::Code::H3_INTERNAL_ERROR, "request sender channel closed"));
					}
				}
			}
		}
	}
}

impl<C, B> ConnectionDriver<C, B>
where
	C: quic::Connection<B>,
	B: Buf,
{
	/// Closes the connection with a code and a reason.
	pub fn close(&mut self, code: h3::error::Code, reason: &str) -> h3::Error {
		self.inner.close(code, reason)
	}
}

/// Accepts incoming requests
pub struct Incoming<C, B>
where
	C: quic::Connection<B>,
	B: Buf,
{
	#[allow(clippy::type_complexity)]
	recv: mpsc::Receiver<(Request<()>, RequestStream<C::BidiStream, B>)>,
}

impl<C, B, E, E2> Connection<C, B>
where
	C: quic::Connection<B> + quic::SendDatagramExt<B, Error = E> + quic::RecvDatagramExt<Buf = B, Error = E2> + 'static,
	B: Buf + 'static + Send + Sync,
	C::AcceptError: Send + Sync,
	C::BidiStream: Send + Sync,
	C::RecvStream: Send + Sync,
	C::OpenStreams: Send + Sync,
	E: Into<h3::Error>,
	E2: Into<h3::Error>,
{
	/// Create a new `WebTransportServer`
	pub fn new(inner: h3::server::Connection<C, B>) -> Self {
		let (request_sender, request_recv) = mpsc::channel(128);
		let (webtransport_request_tx, webtransport_request_rx) = mpsc::channel(128);
		let (session_close_tx, session_close_rx) = mpsc::unbounded_channel();

		Self {
			driver: ConnectionDriver {
				webtransport_session_map: HashMap::new(),
				request_sender,
				webtransport_request_rx,
				webtransport_request_tx,
				session_close_rx,
				session_close_tx,
				inner,
			},
			incoming: Incoming { recv: request_recv },
		}
	}

	/// Take the request acceptor
	pub fn split(self) -> (Incoming<C, B>, ConnectionDriver<C, B>) {
		(self.incoming, self.driver)
	}

	/// Get a mutable reference to the driver
	pub fn driver(&mut self) -> &mut ConnectionDriver<C, B> {
		&mut self.driver
	}

	/// Accepts an incoming request
	/// Internally this method will drive the server until an incoming request
	/// is available And returns the request and a request stream.
	pub async fn accept(&mut self) -> Result<Option<(Request<()>, RequestStream<C::BidiStream, B>)>, h3::Error> {
		match futures_util::future::select(std::pin::pin!(self.incoming.accept()), std::pin::pin!(self.driver.drive())).await
		{
			Either::Left((accept, _)) => Ok(accept),
			Either::Right((drive, _)) => drive.map(|_| None),
		}
	}
}

impl<C, B> Connection<C, B>
where
	C: quic::Connection<B>,
	B: Buf,
{
	/// Closes the connection with a code and a reason.
	pub fn close(&mut self, code: h3::error::Code, reason: &str) -> h3::Error {
		self.driver.close(code, reason)
	}
}

impl<C, B> Incoming<C, B>
where
	C: quic::Connection<B>,
	B: Buf,
{
	/// Accept an incoming request
	pub async fn accept(&mut self) -> Option<(Request<()>, RequestStream<C::BidiStream, B>)> {
		self.recv.recv().await
	}

	/// Poll the request acceptor
	#[allow(clippy::type_complexity)]
	pub fn poll_accept(&mut self, cx: &mut Context<'_>) -> Poll<Option<(Request<()>, RequestStream<C::BidiStream, B>)>> {
		self.recv.poll_recv(cx)
	}
}

fn validate_wt_connect(request: &Request<()>) -> bool {
	let protocol = request.extensions().get::<Protocol>();
	matches!((request.method(), protocol), (&Method::CONNECT, Some(p)) if p == &Protocol::WEB_TRANSPORT)
}
