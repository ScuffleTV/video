use std::sync::Arc;

use futures::future::Either;
use http::HeaderValue;

use super::config::{TcpServerConfigInner, TlsAcceptor};
use super::{util, TcpServerError};
use crate::body::{has_body, Tracker};
use crate::svc::{ConnectionAcceptor, ConnectionHandle, IncommingConnection};
use crate::util::{TimeoutTracker, TimeoutTrackerDropGuard};

pub(super) async fn serve_tcp<S: ConnectionAcceptor + Clone>(
	listener: std::net::TcpListener,
	service: S,
	tls_acceptor: Option<TlsAcceptor>,
	config: TcpServerConfigInner,
) -> Result<(), TcpServerError> {
	let listener = tokio::net::TcpListener::from_std(listener)?;

	loop {
		let (stream, addr) = match listener.accept().await {
			Ok(stream) => stream,
			Err(e) if !util::is_fatal_tcp_error(&e) => continue,
			Err(e) => return Err(TcpServerError::Io(e)),
		};

		let Some(handle) = service.accept(IncommingConnection { addr }) else {
			continue;
		};

		tokio::spawn(serve_stream(stream, addr, handle, tls_acceptor.clone(), config.clone()));
	}
}

async fn serve_stream(
	stream: tokio::net::TcpStream,
	addr: std::net::SocketAddr,
	handle: impl ConnectionHandle,
	tls_acceptor: Option<TlsAcceptor>,
	config: TcpServerConfigInner,
) {
	async fn serve_stream_inner(
		stream: tokio::net::TcpStream,
		addr: std::net::SocketAddr,
		handle: &Arc<impl ConnectionHandle>,
		tls_acceptor: Option<TlsAcceptor>,
		config: TcpServerConfigInner,
	) -> Result<(), crate::Error> {
		handle.accept(IncommingConnection { addr }).await.map_err(Into::into)?;

		match tls_acceptor {
			#[cfg(feature = "tls-rustls")]
			Some(acceptor) => {
				use crate::error::ResultErrorExt;

				// We should read a bit of the stream to see if they are attempting to use TLS
				// or not. This is so we can immediately return a bad request if they arent
				// using TLS.
				let mut stream = stream;
				let is_tls = util::is_tls(&mut stream, handle);

				if !match config.handshake_timeout {
					None => is_tls.await,
					Some(timeout) => tokio::time::timeout(timeout, is_tls).await.unwrap_or(false),
				} {
					return Ok(());
				}

				let lazy = tokio_rustls::LazyConfigAcceptor::new(Default::default(), stream);

				let accepted = match config.handshake_timeout {
					None => lazy.await.with_context("tls handshake")?,
					Some(timeout) => match tokio::time::timeout(timeout, lazy).await {
						Ok(v) => v.with_context("tls handshake")?,
						Err(_) => return Ok(()),
					},
				};

				let Some(tls_config) = acceptor.accept(accepted.client_hello()).await else {
					return Ok(());
				};

				let stream = match config.handshake_timeout {
					None => accepted.into_stream(tls_config).await.with_context("tls handshake")?,
					Some(timeout) => match tokio::time::timeout(timeout, accepted.into_stream(tls_config)).await {
						Ok(v) => v.with_context("tls handshake")?,
						Err(_) => return Ok(()),
					},
				};

				serve_handle(stream, addr, handle, config).await
			}
			None => serve_handle(stream, addr, handle, config).await,
		}
	}

	let handle = Arc::new(handle);
	if let Err(err) = serve_stream_inner(stream, addr, &handle, tls_acceptor, config).await {
		handle.on_error(err);
	}

	handle.on_close();
}

struct DropTracker {
	_guard: Option<TimeoutTrackerDropGuard>,
}

impl Tracker for DropTracker {
	type Error = crate::Error;
}

async fn serve_handle(
	stream: impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + Unpin + 'static,
	addr: std::net::SocketAddr,
	handle: &Arc<impl ConnectionHandle>,
	config: TcpServerConfigInner,
) -> Result<(), crate::Error> {
	tracing::info!("serving connection");

	let io = hyper_util::rt::TokioIo::new(stream);

	let timeout_tracker = config.idle_timeout.map(TimeoutTracker::new).map(Arc::new);

	let service = hyper::service::service_fn(|req: hyper::Request<hyper::body::Incoming>| {
		let guard = timeout_tracker.as_ref().map(|t| t.new_guard());
		let handle = handle.clone();
		let has_body = has_body(req.method());
		let mut req = req.map(|body| {
			if has_body {
				crate::body::IncomingBody::new(body)
			} else {
				crate::body::IncomingBody::empty()
			}
		});

		req.extensions_mut().insert(addr);
		let server_name = config.server_name.clone();
		async move {
			match handle.on_request(req).await {
				Ok(res) => {
					let mut res = res.map(|body| crate::body::TrackedBody::new(body, DropTracker { _guard: guard }));
					if let Some(server_name) = server_name.as_ref() {
						res.headers_mut()
							.insert(hyper::header::SERVER, HeaderValue::from_str(server_name).unwrap());
					}

					Ok(res)
				}
				Err(e) => Err(e.into()),
			}
		}
	});

	let conn = async {
		if config.allow_upgrades {
			config.http_builder.serve_connection_with_upgrades(io, service).await
		} else {
			config.http_builder.serve_connection(io, service).await
		}
	};

	match futures::future::select(
		std::pin::pin!(conn),
		std::pin::pin!(async {
			if let Some(timeout_tracker) = timeout_tracker.as_ref() {
				timeout_tracker.wait().await
			} else {
				std::future::pending().await
			}
		}),
	)
	.await
	{
		Either::Left((e, _)) => {
			if let Err(e) = e {
				handle.on_error(crate::error::downcast(e).with_context("hyper"));
			}
		}
		Either::Right(_) => {}
	}

	handle.on_close();

	Ok(())
}
