use std::io;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use bytes::Bytes;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::StatusCode;
use hyper_util::rt::TokioIo;
use scuffle_utils::context::ContextExt;
use scuffle_utils::http::router::middleware::{CorsMiddleware, CorsOptions};
use scuffle_utils::http::router::Router;
use scuffle_utils::http::RouteError;
use scuffle_utils::prelude::FutureTimeout;
use tokio::net::TcpSocket;

use crate::config::EdgeConfig;
use crate::global::EdgeGlobal;

mod error;
mod stream;

pub use error::EdgeError;

type Body = Full<Bytes>;

pub fn routes<G: EdgeGlobal>(global: &Arc<G>) -> Router<Incoming, Body, RouteError<EdgeError>> {
	let weak = Arc::downgrade(global);
	Router::builder()
		.data(weak)
		.middleware(CorsMiddleware::new(&CorsOptions::wildcard()))
		.error_handler(utils::http::error_handler::<EdgeError, _>)
		.scope("/", stream::routes(global))
		.not_found(|_| async move { Err((StatusCode::NOT_FOUND, "not found").into()) })
		.build()
}

pub async fn run<G: EdgeGlobal>(global: Arc<G>) -> anyhow::Result<()> {
	let config = global.config::<EdgeConfig>();
	tracing::info!("Edge(HTTP) listening on {}", config.bind_address);
	let socket = if config.bind_address.is_ipv6() {
		TcpSocket::new_v6()?
	} else {
		TcpSocket::new_v4()?
	};

	socket.set_reuseaddr(true)?;
	socket.set_reuseport(true)?;
	socket.bind(config.bind_address)?;
	let listener = socket.listen(1024)?;

	let tls_acceptor = if let Some(tls) = &config.tls {
		tracing::info!("TLS enabled");
		let cert = tokio::fs::read(&tls.cert).await.context("failed to read edge ssl cert")?;
		let key = tokio::fs::read(&tls.key)
			.await
			.context("failed to read edge ssl private key")?;

		let key = rustls_pemfile::pkcs8_private_keys(&mut io::BufReader::new(io::Cursor::new(key)))
			.next()
			.ok_or_else(|| anyhow::anyhow!("failed to find private key in edge ssl private key file"))??;

		let certs = rustls_pemfile::certs(&mut io::BufReader::new(io::Cursor::new(cert))).collect::<Result<Vec<_>, _>>()?;

		Some(Arc::new(tokio_rustls::TlsAcceptor::from(Arc::new(
			rustls::ServerConfig::builder()
				.with_no_client_auth()
				.with_single_cert(certs, key.into())?,
		))))
	} else {
		None
	};

	// The reason we use a Weak reference to the global state is because we don't
	// want to block the shutdown When a keep-alive connection is open, the request
	// service will still be alive, and will still be holding a reference to the
	// global state If we used an Arc, the global state would never be dropped, and
	// the shutdown would never complete By using a Weak reference, we can check if
	// the global state is still alive, and if it isn't, we can stop accepting new
	// connections
	let router = Arc::new(routes(&global));

	while let Ok(r) = listener.accept().context(global.ctx()).await {
		let (socket, addr) = r?;

		let router = router.clone();
		let service = service_fn(move |mut req| {
			req.extensions_mut().insert(addr);
			let this = router.clone();
			async move { this.handle(req).await }
		});

		let tls_acceptor = tls_acceptor.clone();

		tracing::debug!("Accepted connection from {}", addr);

		tokio::spawn(async move {
			let http = http1::Builder::new();

			if let Some(tls_acceptor) = tls_acceptor {
				let Ok(Ok(socket)) = tls_acceptor.accept(socket).timeout(Duration::from_secs(5)).await else {
					return;
				};
				tracing::debug!("TLS handshake complete");
				http.serve_connection(TokioIo::new(socket), service)
					.with_upgrades()
					.await
					.ok();
			} else {
				http.serve_connection(TokioIo::new(socket), service)
					.with_upgrades()
					.await
					.ok();
			}
		});
	}

	Ok(())
}
