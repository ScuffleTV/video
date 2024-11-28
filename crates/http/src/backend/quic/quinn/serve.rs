use std::sync::Arc;

use bytes::Bytes;
use h3::server::RequestStream;
use http::Response;
use scuffle_context::ContextFutExt;
#[cfg(feature = "http3-webtransport")]
use scuffle_h3_webtransport::server::WebTransportUpgradePending;

use super::config::{QuinnAcceptorVerdict, QuinnServerConfigInner};
use super::QuinnServerError;
use crate::backend::quic::body::{copy_body, QuicIncomingBodyInner};
use crate::backend::quic::QuicIncomingBody;
use crate::body::{has_body, IncomingBody};
use crate::error::{ErrorScope, ResultErrorExt};
use crate::svc::{ConnectionAcceptor, ConnectionHandle};
use crate::util::TimeoutTracker;
use crate::Error;

pub async fn serve_quinn(
	endpoint: quinn::Endpoint,
	service: impl ConnectionAcceptor,
	config: Arc<QuinnServerConfigInner>,
) -> Result<(), QuinnServerError> {
	serve_quinn_inner(endpoint, service, config).await
}

async fn serve_quinn_inner(
	endpoint: quinn::Endpoint,
	service: impl ConnectionAcceptor,
	config: Arc<QuinnServerConfigInner>,
) -> Result<(), QuinnServerError> {
	while let Some(conn) = endpoint.accept().await {
		let Some(handle) = service.accept() else {
			continue;
		};

		tokio::spawn(serve_handle(conn, handle, config.clone()));
	}

	Ok(())
}

enum Stream {
	Bidi(RequestStream<h3_quinn::BidiStream<Bytes>, Bytes>),
	Send(RequestStream<h3_quinn::SendStream<Bytes>, Bytes>),
}

async fn serve_handle(conn: quinn::Incoming, handle: impl ConnectionHandle, config: Arc<QuinnServerConfigInner>) {
	let handle = Arc::new(handle);

	let ip_addr = conn.remote_address().ip();

	let r = if let Some(acceptor) = &config.quinn_dynamic_config {
		match acceptor.accept().await {
			QuinnAcceptorVerdict::Accept(Some(config)) => conn.accept_with(config),
			QuinnAcceptorVerdict::Accept(None) => conn.accept(),
			QuinnAcceptorVerdict::Refuse => {
				conn.refuse();
				handle.on_close();
				return;
			}
			QuinnAcceptorVerdict::Ignore => {
				conn.ignore();
				handle.on_close();
				return;
			}
		}
	} else {
		conn.accept()
	};

	let conn = match r {
		Ok(conn) => conn,
		Err(err) => {
			handle.on_error(
				Error::from(err)
					.with_scope(ErrorScope::Connection)
					.with_context("quinn accept"),
			);
			handle.on_close();
			return;
		}
	};

	let fut = async {
		if let Some(timeout) = config.handshake_timeout {
			tokio::time::timeout(timeout, conn).await
		} else {
			Ok(conn.await)
		}
	};

	let connection = match fut.await {
		Ok(Ok(connection)) => connection,
		Ok(Err(err)) => {
			handle.on_error(
				Error::from(err)
					.with_scope(ErrorScope::Connection)
					.with_context("quinn handshake"),
			);
			handle.on_close();
			return;
		}
		Err(_) => {
			handle.on_close();
			return;
		}
	};

	let ctx_handler = scuffle_context::Handler::new();

	let fut = async {
		let fut = config.http_builder.build::<_, Bytes>(h3_quinn::Connection::new(connection));

		if let Some(timeout) = config.handshake_timeout {
			tokio::time::timeout(timeout, fut).await
		} else {
			Ok(fut.await)
		}
	};

	let h3_connection = match fut.await {
		Ok(Ok(h3_connection)) => h3_connection,
		Ok(Err(err)) => {
			handle.on_error(
				Error::from(err)
					.with_scope(ErrorScope::Connection)
					.with_context("quinn handshake"),
			);
			handle.on_close();
			return;
		}
		Err(_) => {
			handle.on_close();
			return;
		}
	};

	#[cfg(feature = "http3-webtransport")]
	enum WebTransportWrapper {
		Connection(h3::server::Connection<h3_quinn::Connection, Bytes>),
		WebTransport(scuffle_h3_webtransport::server::Connection<h3_quinn::Connection, Bytes>),
	}

	#[cfg(feature = "http3-webtransport")]
	impl WebTransportWrapper {
		async fn accept(
			&mut self,
		) -> Result<Option<(http::Request<()>, RequestStream<h3_quinn::BidiStream<Bytes>, Bytes>)>, h3::Error> {
			match self {
				Self::Connection(conn) => conn.accept().await,
				Self::WebTransport(conn) => conn.accept().await,
			}
		}
	}

	#[cfg(feature = "http3-webtransport")]
	let mut h3_connection = if h3_connection.inner.config.settings.enable_webtransport() {
		WebTransportWrapper::WebTransport(scuffle_h3_webtransport::server::Connection::new(h3_connection))
	} else {
		WebTransportWrapper::Connection(h3_connection)
	};

	#[cfg(not(feature = "http3-webtransport"))]
	let mut h3_connection = h3_connection;

	let timeout_tracker = config.idle_timeout.map(|timeout| Arc::new(TimeoutTracker::new(timeout)));

	loop {
		let timeout_fut = async {
			if let Some(timeout_tracker) = &timeout_tracker {
				timeout_tracker.wait().await;
			} else {
				std::future::pending().await
			}
		};

		let conn = match futures::future::select(std::pin::pin!(h3_connection.accept()), std::pin::pin!(timeout_fut)).await {
			futures::future::Either::Left((conn, _)) => conn,
			futures::future::Either::Right((_, _)) => {
				break;
			}
		};

		let (request, stream) = match conn {
			Ok(Some((request, stream))) => (request, stream),
			Ok(None) => {
				tracing::debug!("no request, closing connection");
				break;
			}
			Err(err) => {
				handle.on_error(
					Error::from(err)
						.with_scope(ErrorScope::Connection)
						.with_context("quinn accept"),
				);
				break;
			}
		};

		let (send, mut request) = if has_body(request.method()) {
			let (send, recv) = stream.split();

			let size_hint = request
				.headers()
				.get(http::header::CONTENT_LENGTH)
				.and_then(|len| len.to_str().ok().and_then(|x| x.parse().ok()));
			(
				Stream::Send(send),
				request.map(|()| IncomingBody::new(QuicIncomingBody::Quinn(QuicIncomingBodyInner::new(recv, size_hint)))),
			)
		} else {
			(Stream::Bidi(stream), request.map(|_| IncomingBody::empty()))
		};

		let ctx = ctx_handler.context();

		let handle = handle.clone();
		let timeout_guard = timeout_tracker.as_ref().map(|tracker| tracker.new_guard());
		request.extensions_mut().insert(ip_addr);

		tokio::spawn(
			async move {
				if let Err(err) = handle_request(&handle, request, send).await {
					handle.on_error(err.with_scope(ErrorScope::Request));
				}

				drop(timeout_guard);
			}
			.with_context(ctx),
		);
	}

	ctx_handler.shutdown().await;
	handle.on_close();
}

async fn handle_request(
	handle: &Arc<impl ConnectionHandle>,
	request: http::Request<IncomingBody>,
	send: Stream,
) -> Result<(), crate::Error> {
	let response = handle.on_request(request).await.map_err(Into::into)?;

	#[cfg(feature = "http3-webtransport")]
	let (mut response, mut send) = (response, send);

	#[cfg(feature = "http3-webtransport")]
	if let Some(pending) = response
		.extensions_mut()
		.remove::<WebTransportUpgradePending<h3_quinn::Connection, Bytes>>()
	{
		let result = match send {
			Stream::Bidi(stream) => pending.upgrade(stream).map_err(Stream::Bidi),
			Stream::Send(stream) => Err(Stream::Send(stream)),
		};

		match result {
			Ok(upgraded) => return upgraded.await.with_context("http3 webtransport upgrade"),
			Err(stream) => send = stream,
		}
	}

	let (parts, body) = response.into_parts();
	let response = Response::from_parts(parts, ());

	let mut send = match send {
		Stream::Bidi(stream) => stream.split().0,
		Stream::Send(stream) => stream,
	};

	send.send_response(response).await.with_context("send response")?;

	copy_body(send, body).await.with_context("copy body")?;

	Ok(())
}
