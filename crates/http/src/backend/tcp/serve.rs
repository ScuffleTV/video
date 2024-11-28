use std::sync::Arc;

use super::config::{TcpServerConfigInner, TlsAcceptor};
use super::{util, TcpServerError};
use crate::error::ResultErrorExt;
use crate::svc::{ConnectionAcceptor, ConnectionHandle};

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

		let Some(handle) = service.accept() else {
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
		match tls_acceptor {
			#[cfg(feature = "tls-rustls")]
			Some(acceptor) => {
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

async fn serve_handle(
	stream: impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + Unpin + 'static,
	addr: std::net::SocketAddr,
	handle: &Arc<impl ConnectionHandle>,
	config: TcpServerConfigInner,
) -> Result<(), crate::Error> {
	tracing::info!("serving connection");

	#[cfg(all(feature = "http1", feature = "http2"))]
	serve_either_http1_or_http2(stream, addr, handle, config).await?;

	#[cfg(all(feature = "http1", not(feature = "http2")))]
	super::http1::serve(stream, addr, &handle, config)
		.await
		.with_context("http1")?;

	#[cfg(all(not(feature = "http1"), feature = "http2"))]
	super::http2::serve(stream, addr, &handle, config)
		.await
		.with_context("http2")?;

	Ok(())
}

#[cfg(all(feature = "http1", feature = "http2"))]
async fn serve_either_http1_or_http2(
	stream: impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + Unpin + 'static,
	addr: std::net::SocketAddr,
	handle: &Arc<impl ConnectionHandle>,
	config: TcpServerConfigInner,
) -> Result<(), crate::Error> {
	match (&config.http1_builder, &config.http2_builder) {
		(Some(_), Some(_)) => {
			let fut = util::read_version(stream);
			let (version, rewind) = match config.idle_timeout {
				None => fut.await.with_context("read version")?,
				Some(timeout) => tokio::time::timeout(timeout, fut)
					.await
					.with_context("read version timeout")?
					.with_context("read version")?,
			};

			match version {
				util::Version::H1 => super::http1::serve(rewind, addr, handle, config).await.with_context("http1"),
				util::Version::H2 => super::http2::serve(rewind, addr, handle, config).await.with_context("http2"),
			}
		}
		(Some(_), None) => super::http1::serve(stream, addr, handle, config).await.with_context("http1"),
		(None, Some(_)) => super::http2::serve(stream, addr, handle, config).await.with_context("http2"),
		(None, None) => Err(crate::Error::from("No HTTP/1 or HTTP/2 builder configured")),
	}
}
