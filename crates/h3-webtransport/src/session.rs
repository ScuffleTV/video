use std::future::poll_fn;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use bytes::Buf;
use futures_util::future::BoxFuture;
use h3::quic::{self, OpenStreams};
use h3::server::RequestStream;
use h3::stream::BufRecvStream;
use h3::webtransport::SessionId;
use http::{Request, Response, StatusCode};
use tokio::sync::{mpsc, oneshot};

use crate::server::{WebTransportCanUpgrade, WebTransportRequest, WebTransportUpgradePending};
use crate::stream::{BidiStream, RecvStream};

/// WebTransport session driver.
///
/// Maintains the session using the underlying HTTP/3 connection.
///
/// Similar to [`h3::server::Connection`](https://docs.rs/h3/latest/h3/server/struct.Connection.html) it is generic over the QUIC implementation and Buffer.
pub struct WebTransportSession<C, B>
where
	C: quic::Connection<B>,
	B: Buf,
{
	// See: https://datatracker.ietf.org/doc/html/draft-ietf-webtrans-http3/#section-2-3
	connect_stream: RequestStream<C::BidiStream, B>,
	webtransport_request_tx: mpsc::Sender<WebTransportRequest<C, B>>,
	session_close_tx: mpsc::UnboundedSender<SessionId>,
	inner: Mutex<Inner<C, B>>,
}

struct Inner<C, B>
where
	C: quic::Connection<B>,
	B: Buf,
{
	opener: C::OpenStreams,
	bidi_request_rx: mpsc::Receiver<BidiStream<C::BidiStream, B>>,
	uni_request_rx: mpsc::Receiver<RecvStream<C::RecvStream, B>>,
	datagram_request_rx: mpsc::Receiver<B>,
}

impl<C, B> Drop for WebTransportSession<C, B>
where
	C: quic::Connection<B>,
	B: Buf,
{
	fn drop(&mut self) {
		self.session_close_tx.send(self.session_id()).ok();
		self.connect_stream.stop_sending(h3::error::Code::H3_NO_ERROR);
		self.connect_stream.stop_stream(h3::error::Code::H3_NO_ERROR);
	}
}

impl<C, B> WebTransportSession<C, B>
where
	C: quic::Connection<B>,
	B: Buf,
{
	async fn new(
		mut stream: RequestStream<C::BidiStream, B>,
		webtransport_request_tx: mpsc::Sender<WebTransportRequest<C, B>>,
	) -> Result<Option<Self>, h3::Error> {
		let (bidi_tx, bidi_rx) = mpsc::channel(16);
		let (uni_tx, uni_rx) = mpsc::channel(16);
		let (datagram_tx, datagram_rx) = mpsc::channel(16);

		let (tx, rx) = oneshot::channel();

		if webtransport_request_tx
			.send(WebTransportRequest::Upgrade {
				session_id: stream.id().into(),
				bidi_request: bidi_tx,
				uni_request: uni_tx,
				datagram_request: datagram_tx,
				response: tx,
			})
			.await
			.is_err()
		{
			stream
				.send_response(
					http::Response::builder()
						.status(StatusCode::INTERNAL_SERVER_ERROR)
						.body(())
						.unwrap(),
				)
				.await?;
			return Ok(None);
		}

		let Ok((opener, session_close_tx)) = rx.await else {
			stream
				.send_response(
					http::Response::builder()
						.status(StatusCode::INTERNAL_SERVER_ERROR)
						.body(())
						.unwrap(),
				)
				.await?;
			return Ok(None);
		};

		stream
			.send_response(
				http::Response::builder()
					// This is the only header that chrome cares about.
					.header("sec-webtransport-http3-draft", "draft02")
					.status(StatusCode::OK)
					.body(())
					.unwrap(),
			)
			.await?;

		Ok(Some(Self {
			connect_stream: stream,
			webtransport_request_tx,
			session_close_tx,
			inner: Mutex::new(Inner {
				opener,
				bidi_request_rx: bidi_rx,
				uni_request_rx: uni_rx,
				datagram_request_rx: datagram_rx,
			}),
		}))
	}

	/// Returns the session id
	pub fn session_id(&self) -> SessionId {
		self.connect_stream.id().into()
	}

	/// Accepts a bidi stream
	pub async fn accept_bi(&self) -> Option<BidiStream<C::BidiStream, B>> {
		poll_fn(|cx| self.poll_accept_bi(cx)).await
	}

	/// Polls for a bidi stream
	pub fn poll_accept_bi(&self, cx: &mut Context<'_>) -> Poll<Option<BidiStream<C::BidiStream, B>>> {
		self.inner.lock().unwrap().bidi_request_rx.poll_recv(cx)
	}

	/// Accepts a uni stream
	pub async fn accept_uni(&self) -> Option<RecvStream<C::RecvStream, B>> {
		poll_fn(|cx| self.poll_accept_uni(cx)).await
	}

	/// Polls for a uni stream
	pub fn poll_accept_uni(&self, cx: &mut Context<'_>) -> Poll<Option<RecvStream<C::RecvStream, B>>> {
		self.inner.lock().unwrap().uni_request_rx.poll_recv(cx)
	}

	/// Accepts a datagram
	pub async fn accept_datagram(&self) -> Option<B> {
		poll_fn(|cx| self.poll_accept_datagram(cx)).await
	}

	/// Polls for a datagram
	pub fn poll_accept_datagram(&self, cx: &mut Context<'_>) -> Poll<Option<B>> {
		self.inner.lock().unwrap().datagram_request_rx.poll_recv(cx)
	}

	/// Sends a datagram
	pub async fn send_datagram(&self, datagram: B) -> Result<(), h3::Error> {
		let (tx, rx) = oneshot::channel();
		self.webtransport_request_tx
			.send(WebTransportRequest::SendDatagram {
				session_id: self.session_id(),
				datagram,
				resp: tx,
			})
			.await
			.ok();

		match rx.await {
			Ok(Ok(())) => Ok(()),
			Ok(Err(e)) => Err(e),
			// If the channel is closed, we can ignore the error
			Err(_) => Ok(()),
		}
	}

	/// Opens a bidi stream
	pub async fn open_bi(
		&self,
	) -> Result<crate::stream::BidiStream<C::BidiStream, B>, <C::OpenStreams as OpenStreams<B>>::OpenError> {
		poll_fn(|cx| self.poll_open_bi(cx)).await
	}

	/// Polls to open a bidi stream
	#[allow(clippy::type_complexity)]
	pub fn poll_open_bi(
		&self,
		cx: &mut Context<'_>,
	) -> Poll<Result<crate::stream::BidiStream<C::BidiStream, B>, <C::OpenStreams as OpenStreams<B>>::OpenError>> {
		self.inner
			.lock()
			.unwrap()
			.opener
			.poll_open_bidi(cx)
			.map(|res| res.map(|stream| crate::stream::BidiStream::new(BufRecvStream::new(stream))))
	}

	/// Opens a uni stream
	pub async fn open_uni(
		&self,
	) -> Result<crate::stream::SendStream<C::SendStream, B>, <C::OpenStreams as OpenStreams<B>>::OpenError> {
		poll_fn(|cx| self.poll_open_uni(cx)).await
	}

	/// Polls to open a uni stream
	#[allow(clippy::type_complexity)]
	pub fn poll_open_uni(
		&self,
		cx: &mut Context<'_>,
	) -> Poll<Result<crate::stream::SendStream<C::SendStream, B>, <C::OpenStreams as OpenStreams<B>>::OpenError>> {
		self.inner
			.lock()
			.unwrap()
			.opener
			.poll_open_send(cx)
			.map(|res| res.map(|stream| crate::stream::SendStream::new(BufRecvStream::new(stream))))
	}
}

impl<C, B> WebTransportSession<C, B>
where
	C: quic::Connection<B>,
	B: Buf,
{
	/// Begin a WebTransport session upgrade
	pub fn begin<B2, F, Fut>(request: &mut Request<B2>, on_upgrade: F) -> Option<http::Response<()>>
	where
		C: quic::Connection<B> + 'static,
		B: Buf + 'static + Send + Sync,
		C::AcceptError: Send + Sync,
		C::BidiStream: Send + Sync,
		C::RecvStream: Send + Sync,
		C::OpenStreams: Send + Sync,
		Fut: std::future::Future<Output = ()> + Send + Sync + 'static,
		F: FnOnce(WebTransportSession<C, B>) -> Fut + Send + Sync + 'static,
	{
		let can_upgrade = request.extensions_mut().remove::<WebTransportCanUpgrade<C, B>>()?;

		let resp = Response::builder()
			.extension(WebTransportUpgradePending::<C, B> {
				complete_upgrade: Arc::new(Mutex::new(Some(Box::new(move |stream| {
					Box::pin(async move {
						let Some(session) = WebTransportSession::new(stream, can_upgrade.webtransport_request_tx).await?
						else {
							return Ok(());
						};

						on_upgrade(session).await;
						Ok(())
					})
				})))),
			})
			.status(StatusCode::BAD_REQUEST)
			.body(())
			.unwrap();

		Some(resp)
	}

	/// Completes the WebTransport upgrade
	#[allow(clippy::type_complexity)]
	pub fn complete(
		response: &mut Response<B>,
		stream: RequestStream<C::BidiStream, B>,
	) -> Result<BoxFuture<'static, Result<(), h3::Error>>, RequestStream<C::BidiStream, B>>
	where
		C: quic::Connection<B> + 'static,
		B: Buf + 'static + Send + Sync,
	{
		let Some(upgrade_pending) = response.extensions_mut().remove::<WebTransportUpgradePending<C, B>>() else {
			return Err(stream);
		};

		upgrade_pending.upgrade(stream)
	}

	/// Accepts a WebTransport session from an incoming request
	pub async fn accept<E, E2>(
		request: &mut Request<()>,
		mut stream: RequestStream<C::BidiStream, B>,
	) -> Result<Option<WebTransportSession<C, B>>, h3::Error>
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
		let Some(can_upgrade) = request.extensions_mut().remove::<WebTransportCanUpgrade<C, B>>() else {
			stream
				.send_response(http::Response::builder().status(StatusCode::BAD_REQUEST).body(()).unwrap())
				.await?;
			stream.finish().await?;
			return Ok(None);
		};

		WebTransportSession::new(stream, can_upgrade.webtransport_request_tx).await
	}
}
