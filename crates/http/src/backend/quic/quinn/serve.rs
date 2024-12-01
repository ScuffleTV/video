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
use crate::error::{ErrorConfig, ErrorScope, ErrorSeverity, ResultErrorExt};
use crate::svc::{ConnectionAcceptor, ConnectionHandle, IncomingConnection};
use crate::util::TimeoutTracker;

pub async fn serve_quinn(
	endpoint: quinn::Endpoint,
	service: impl ConnectionAcceptor,
	config: Arc<QuinnServerConfigInner>,
	ctx: scuffle_context::Context,
) -> Result<(), QuinnServerError> {
	let (ctx, ctx_handler) = ctx.new_child();

	serve_quinn_inner(endpoint, service, config, ctx).await?;

	ctx_handler.shutdown().await;

	Ok(())
}

async fn serve_quinn_inner(
	endpoint: quinn::Endpoint,
	service: impl ConnectionAcceptor,
	config: Arc<QuinnServerConfigInner>,
	ctx: scuffle_context::Context,
) -> Result<(), QuinnServerError> {
	while let Some(Some(conn)) = endpoint.accept().with_context(&ctx).await {
		let Some(handle) = service.accept(IncomingConnection {
			addr: conn.remote_address(),
		}) else {
			continue;
		};

		tokio::spawn(serve_handle(conn, handle, config.clone(), ctx.clone()));
	}

	Ok(())
}

enum Stream {
	Bidi(RequestStream<h3_quinn::BidiStream<Bytes>, Bytes>),
	Send(RequestStream<h3_quinn::SendStream<Bytes>, Bytes>),
}

async fn serve_handle(
	conn: quinn::Incoming,
	handle: impl ConnectionHandle,
	config: Arc<QuinnServerConfigInner>,
	ctx: scuffle_context::Context,
) {
	tracing::debug!("serving quinn connection: {:?}", conn.remote_address());
	let handle = Arc::new(handle);

	let (ctx, ctx_handler) = ctx.new_child();

	if let Err(err) = serve_handle_inner(conn, &handle, config, ctx).await {
		handle.on_error(err.with_scope(ErrorScope::Connection).with_context("quinn accept"));
	}

	ctx_handler.shutdown().await;

	handle.on_close();
}

async fn serve_handle_inner(
	conn: quinn::Incoming,
	handle: &Arc<impl ConnectionHandle>,
	config: Arc<QuinnServerConfigInner>,
	ctx: scuffle_context::Context,
) -> Result<(), crate::Error> {
	handle
		.accept(IncomingConnection {
			addr: conn.remote_address(),
		})
		.with_context(&ctx)
		.await
		.transpose()
		.map_err(Into::into)?;

	let ip_addr = conn.remote_address().ip();

	let conn = if let Some(acceptor) = &config.quinn_dynamic_config {
		match acceptor.accept().with_context(&ctx).await {
			Some(QuinnAcceptorVerdict::Accept(Some(config))) => conn.accept_with(config),
			Some(QuinnAcceptorVerdict::Accept(None)) => conn.accept(),
			Some(QuinnAcceptorVerdict::Refuse) => {
				conn.refuse();
				return Ok(());
			}
			Some(QuinnAcceptorVerdict::Ignore) => {
				conn.ignore();
				return Ok(());
			}
			None => {
				conn.refuse();
				return Ok(());
			}
		}
	} else {
		conn.accept()
	}
	.with_config(ErrorConfig {
		context: "quinn accept",
		scope: ErrorScope::Connection,
		severity: ErrorSeverity::Debug,
	})?
	.with_context(&ctx);

	let Some(connection) = if let Some(timeout) = config.handshake_timeout {
		tokio::time::timeout(timeout, conn).await.with_config(ErrorConfig {
			context: "quinn handshake",
			scope: ErrorScope::Connection,
			severity: ErrorSeverity::Debug,
		})?
	} else {
		conn.await
	}
	.transpose()
	.with_config(ErrorConfig {
		context: "quinn handshake",
		scope: ErrorScope::Connection,
		severity: ErrorSeverity::Debug,
	})?
	else {
		return Ok(());
	};

	let Some(h3_connection) = {
		let fut = config
			.http_builder
			.build::<_, Bytes>(h3_quinn::Connection::new(connection))
			.with_context(&ctx);

		if let Some(timeout) = config.handshake_timeout {
			tokio::time::timeout(timeout, fut).await.with_config(ErrorConfig {
				context: "quinn handshake",
				scope: ErrorScope::Connection,
				severity: ErrorSeverity::Debug,
			})?
		} else {
			fut.await
		}
	}
	.transpose()
	.with_config(ErrorConfig {
		context: "quinn handshake",
		scope: ErrorScope::Connection,
		severity: ErrorSeverity::Debug,
	})?
	else {
		return Ok(());
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
	let timeout_fut = async {
		if let Some(timeout_tracker) = &timeout_tracker {
			timeout_tracker.wait().with_context(&ctx).await;
		} else {
			ctx.done().await
		}
	};

	let mut pinned_timeout_fut = std::pin::pin!(timeout_fut);

	loop {
		let conn = match futures::future::select(std::pin::pin!(h3_connection.accept()), pinned_timeout_fut.as_mut()).await {
			futures::future::Either::Left((conn, _)) => conn,
			futures::future::Either::Right((_, _)) => {
				return Ok(());
			}
		};

		let Some((request, stream)) = conn.with_context("quinn accept")? else {
			tracing::debug!("no request, closing connection");
			return Ok(());
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

		let handle = handle.clone();
		let timeout_guard = timeout_tracker.as_ref().map(|tracker| tracker.new_guard());
		request.extensions_mut().insert(ip_addr);

		let ctx = ctx.clone();

		tokio::spawn(async move {
			if let Err(err) = handle_request(&handle, request, send).await {
				handle.on_error(err.with_scope(ErrorScope::Request));
			}

			drop((timeout_guard, ctx));
		});
	}
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
