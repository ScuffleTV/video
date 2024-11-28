use std::sync::Arc;

use bytes::{Buf, Bytes};
use h2::server::SendResponse;
use h2::SendStream;
use http_body::Body;

use super::config::TcpServerConfigInner;
use crate::backend::tcp::TcpIncomingBody;
use crate::body::{has_body, IncomingBody};
use crate::error::{ErrorConfig, ErrorKind, ErrorScope, ErrorSeverity, ResultErrorExt};
use crate::svc::ConnectionHandle;
use crate::util::{TimeoutTracker, TimeoutTrackerDropGuard};
use crate::Error;

pub mod io;

pub async fn serve(
	stream: impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + Unpin + 'static,
	addr: std::net::SocketAddr,
	handle: &Arc<impl ConnectionHandle>,
	config: TcpServerConfigInner,
) -> Result<(), crate::Error> {
	let http2_builder = config.http2_builder.ok_or(ErrorConfig {
		severity: ErrorSeverity::Debug,
		scope: ErrorScope::Connection,
		context: "http2 builder not configured",
	})?;

	let mut connection = http2_builder
		.handshake::<_, bytes::Bytes>(stream)
		.await
		.with_scope(ErrorScope::Connection)
		.with_context("http2 handshake")?;

	let ping_pong = connection.ping_pong().ok_or(ErrorConfig {
		severity: ErrorSeverity::Debug,
		scope: ErrorScope::Connection,
		context: "http2 ping pong",
	})?;

	let shared = io::Http2SharedState::new(ping_pong);

	let ctx_handler = scuffle_context::Handler::new();

	let timeout_tracker = config.idle_timeout.map(|timeout| Arc::new(TimeoutTracker::new(timeout)));

	let mut timeout_fut = std::pin::pin!(async {
		if let Some(timeout_tracker) = &timeout_tracker {
			timeout_tracker.wait().await;
		} else {
			futures::future::pending().await
		}
	});

	let mut timedout = false;

	loop {
		let conn = match futures::future::select(std::pin::pin!(connection.accept()), &mut timeout_fut).await {
			futures::future::Either::Left((conn, _)) => conn,
			futures::future::Either::Right(_) => {
				timedout = true;
				break;
			}
		};

		let Some((mut request, stream)) = conn.transpose().with_config(ErrorConfig {
			severity: ErrorSeverity::Unknown,
			scope: ErrorScope::Connection,
			context: "http2 accept",
		})?
		else {
			break;
		};

		request.extensions_mut().insert(addr);

		let request = if has_body(request.method()) {
			let body_size = request
				.headers()
				.get(http::header::CONTENT_LENGTH)
				.map(|v| {
					v.to_str().ok().and_then(|v| v.parse().ok()).ok_or_else(|| {
						Error::new().with_config(ErrorConfig {
							severity: ErrorSeverity::Debug,
							scope: ErrorScope::Request,
							context: "bad content length header",
						})
					})
				})
				.transpose()?;

			request
				.map(|body| IncomingBody::new(TcpIncomingBody::Http2(io::RecvStream::new(body, body_size, shared.clone()))))
		} else {
			request.map(|_| IncomingBody::empty())
		};

		tokio::spawn(serve_http2_request(
			handle.clone(),
			request,
			stream,
			timeout_tracker.as_ref().map(|t| t.new_guard()),
		));
	}

	if timedout {
		connection.abrupt_shutdown(h2::Reason::NO_ERROR);
		tokio::time::timeout(
			std::time::Duration::from_secs(5),
			std::future::poll_fn(|cx| connection.poll_closed(cx)),
		)
		.await
		.ok();
	}

	ctx_handler.shutdown().await;

	Ok(())
}

async fn serve_http2_request(
	handle: Arc<impl ConnectionHandle>,
	request: http::Request<IncomingBody>,
	mut stream: SendResponse<Bytes>,
	_timeout_guard: Option<TimeoutTrackerDropGuard>,
) {
	let response = match handle.on_request(request).await.map_err(Into::into) {
		Ok(response) => response,
		Err(e) => {
			handle.on_error(e.with_scope(ErrorScope::Request));
			return;
		}
	};

	let (parts, body) = response.into_parts();
	let response = http::Response::from_parts(parts, ());

	let stream = match stream.send_response(response, body.is_end_stream()) {
		Ok(stream) => stream,
		Err(e) => {
			handle.on_error(Error::from(e).with_scope(ErrorScope::Request));
			return;
		}
	};

	if let Err(e) = copy_http2_body(stream, body).await {
		handle.on_error(e.with_scope(ErrorScope::Request));
	}
}

async fn copy_http2_body<E: Into<crate::Error>>(
	mut send: SendStream<Bytes>,
	body: impl Body<Error = E>,
) -> Result<(), crate::Error> {
	let mut body = std::pin::pin!(body);
	let mut closed = false;
	loop {
		while send.capacity() == 0 {
			send.reserve_capacity(1);
			match std::future::poll_fn(|cx| send.poll_capacity(cx)).await {
				Some(Ok(0)) => continue,
				Some(Ok(_)) => break,
				Some(Err(e)) => return Err(Error::from(e).with_context("send stream")),
				None => return Err(Error::with_kind(ErrorKind::Closed).with_context("send stream")),
			}
		}

		let Some(frame) = std::future::poll_fn(|cx| body.as_mut().poll_frame(cx)).await else {
			break;
		};

		match frame {
			Ok(frame) => match frame.into_data().map_err(|f| f.into_trailers()) {
				Ok(mut data) => {
					closed = body.is_end_stream();
					send.send_data(data.copy_to_bytes(data.remaining()), closed)
						.with_context("send data")?;
					if closed {
						break;
					}
				}
				Err(Ok(trailers)) => {
					send.send_trailers(trailers).with_context("send trailers")?;
					closed = true;
					break;
				}
				Err(Err(_)) => continue,
			},
			Err(err) => return Err(err.into().with_context("http2 body")),
		}
	}

	if !closed {
		send.send_data(Bytes::new(), true).with_context("finish")?;
	}

	// Free any remaining capacity
	send.reserve_capacity(0);

	Ok(())
}
