use std::sync::Arc;

use futures::future::Either;
use http::HeaderValue;
use scuffle_context::ContextFutExt;

use super::config::{TcpServerConfigInner, TlsAcceptor};
use super::{util, TcpServerError};
use crate::body::{has_body, Tracker};
use crate::svc::{ConnectionAcceptor, ConnectionHandle, IncomingConnection};
use crate::util::{TimeoutTracker, TimeoutTrackerDropGuard};

pub(super) async fn serve_tcp(
	listener: std::net::TcpListener,
	service: impl ConnectionAcceptor + Clone,
	tls_acceptor: Option<TlsAcceptor>,
	config: TcpServerConfigInner,
	ctx: scuffle_context::Context,
) -> Result<(), TcpServerError> {
	let (ctx, ctx_handler) = ctx.new_child();

	serve_tcp_inner(listener, service, tls_acceptor, config, ctx).await?;

	ctx_handler.shutdown().await;

	Ok(())
}

async fn serve_tcp_inner(
	listener: std::net::TcpListener,
	service: impl ConnectionAcceptor + Clone,
	tls_acceptor: Option<TlsAcceptor>,
	config: TcpServerConfigInner,
	ctx: scuffle_context::Context,
) -> Result<(), TcpServerError> {
	listener.set_nonblocking(true)?;

	let listener = tokio::net::TcpListener::from_std(listener)?;

	loop {
		let (stream, addr) = match listener.accept().with_context(&ctx).await {
			Some(Ok(stream)) => stream,
			Some(Err(e)) if !util::is_fatal_tcp_error(&e) => continue,
			Some(Err(e)) => return Err(TcpServerError::Io(e)),
			None => break,
		};

		let Some(handle) = service.accept(IncomingConnection { addr }) else {
			continue;
		};

		tokio::spawn(serve_stream(
			stream,
			addr,
			handle,
			tls_acceptor.clone(),
			config.clone(),
			ctx.clone(),
		));
	}

	Ok(())
}

async fn serve_stream(
	stream: tokio::net::TcpStream,
	addr: std::net::SocketAddr,
	handle: impl ConnectionHandle,
	tls_acceptor: Option<TlsAcceptor>,
	config: TcpServerConfigInner,
	ctx: scuffle_context::Context,
) {
	let (ctx, ctx_handler) = ctx.new_child();

	let handle = Arc::new(handle);
	if let Err(err) = serve_stream_inner(stream, addr, &handle, tls_acceptor, config, ctx).await {
		handle.on_error(err);
	}

	ctx_handler.shutdown().await;

	handle.on_close();
}

async fn serve_stream_inner(
	stream: tokio::net::TcpStream,
	addr: std::net::SocketAddr,
	handle: &Arc<impl ConnectionHandle>,
	tls_acceptor: Option<TlsAcceptor>,
	config: TcpServerConfigInner,
	ctx: scuffle_context::Context,
) -> Result<(), crate::Error> {
	if handle
		.accept(IncomingConnection { addr })
		.with_context(&ctx)
		.await
		.transpose()
		.map_err(Into::into)?
		.is_none()
	{
		// The ctx expired so we just exit early.
		return Ok(());
	}

	match tls_acceptor {
		#[cfg(feature = "tls-rustls")]
		Some(acceptor) => {
			use crate::error::{ErrorConfig, ErrorKind, ErrorScope, ErrorSeverity, ResultErrorExt};

			let Some(stream) = async {
				// We should read a bit of the stream to see if they are attempting to use TLS
				// or not. This is so we can immediately return a bad request if they arent
				// using TLS.
				let mut stream = stream;
				let is_tls = util::is_tls(&mut stream, handle);

				let is_tls = if let Some(timeout) = config.handshake_timeout {
					tokio::time::timeout(timeout, is_tls).await.with_config(ErrorConfig {
						context: "tls handshake",
						scope: ErrorScope::Connection,
						severity: ErrorSeverity::Debug,
					})?
				} else {
					is_tls.await
				};

				if !is_tls {
					return Err(crate::Error::with_kind(ErrorKind::BadRequest).with_config(ErrorConfig {
						context: "tls handshake",
						scope: ErrorScope::Connection,
						severity: ErrorSeverity::Debug,
					}));
				}

				let lazy = tokio_rustls::LazyConfigAcceptor::new(Default::default(), stream);

				let accepted = if let Some(timeout) = config.handshake_timeout {
					tokio::time::timeout(timeout, lazy).await.with_config(ErrorConfig {
						context: "tls handshake",
						scope: ErrorScope::Connection,
						severity: ErrorSeverity::Debug,
					})?
				} else {
					lazy.await
				}
				.with_config(ErrorConfig {
					context: "tls handshake",
					scope: ErrorScope::Connection,
					severity: ErrorSeverity::Debug,
				})?;

				let Some(tls_config) = acceptor.accept(accepted.client_hello()).await else {
					return Ok(None);
				};

				let stream = if let Some(timeout) = config.handshake_timeout {
					tokio::time::timeout(timeout, accepted.into_stream(tls_config))
						.await
						.with_config(ErrorConfig {
							context: "tls handshake",
							scope: ErrorScope::Connection,
							severity: ErrorSeverity::Debug,
						})?
				} else {
					accepted.into_stream(tls_config).await
				};

				stream.map(Some).with_config(ErrorConfig {
					context: "tls handshake",
					scope: ErrorScope::Connection,
					severity: ErrorSeverity::Debug,
				})
			}
			.with_context(&ctx)
			.await
			.transpose()?
			.flatten() else {
				// Either the ctx expired or the handshake failed so we just exit early.
				return Ok(());
			};

			serve_handle(stream, addr, handle, config, &ctx).await
		}
		#[cfg(not(feature = "tls-rustls"))]
		Some(_) => unreachable!(),
		None => serve_handle(stream, addr, handle, config, &ctx).await,
	}
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
	ctx: &scuffle_context::Context,
) -> Result<(), crate::Error> {
	tracing::debug!("serving connection: {:?}", addr);

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
		let ctx = ctx.clone();

		req.extensions_mut().insert(addr.ip());
		let server_name = config.server_name.clone();
		async move {
			let _ctx = ctx.clone();
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
			let conn = config.http_builder.serve_connection_with_upgrades(io, service);
			let mut pinned = std::pin::pin!(conn);
			if pinned.as_mut().with_context(ctx).await.transpose()?.is_none() {
				pinned.as_mut().graceful_shutdown();
				pinned.await?;
			}
		} else {
			let conn = config.http_builder.serve_connection(io, service);
			let mut pinned = std::pin::pin!(conn);
			if pinned.as_mut().with_context(ctx).await.transpose()?.is_none() {
				pinned.as_mut().graceful_shutdown();
				pinned.await?;
			}
		}

		Ok(())
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

	Ok(())
}
