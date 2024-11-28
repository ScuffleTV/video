use std::io;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use scuffle_utils::context::ContextExt;
use scuffle_utils::prelude::FutureTimeout;
use tokio::net::TcpSocket;

use crate::config::IngestConfig;
use crate::global::IngestGlobal;

mod bytes_tracker;
mod connection;
mod errors;
mod rtmp_session;
mod update;

pub async fn run<G: IngestGlobal>(global: Arc<G>) -> Result<()> {
	let config = global.config::<IngestConfig>();
	tracing::info!("Ingest(RTMP) listening on {}", config.rtmp.bind_address);
	let socket = if config.rtmp.bind_address.is_ipv6() {
		TcpSocket::new_v6()?
	} else {
		TcpSocket::new_v4()?
	};

	socket.set_reuseaddr(true)?;
	socket.set_reuseport(true)?;
	socket.bind(config.rtmp.bind_address)?;
	let listener = socket.listen(1024)?;
	let tls_acceptor = if let Some(tls) = &config.rtmp.tls {
		tracing::info!("TLS enabled");
		let cert = std::fs::read(&tls.cert).expect("failed to read rtmp cert");
		let key = std::fs::read(&tls.key).expect("failed to read rtmp key");

		let key = rustls_pemfile::pkcs8_private_keys(&mut io::BufReader::new(io::Cursor::new(key)))
			.next()
			.ok_or_else(|| anyhow::anyhow!("failed to find private key in rtmp private key file"))??
			.into();

		let certs = rustls_pemfile::certs(&mut io::BufReader::new(io::Cursor::new(cert))).collect::<Result<Vec<_>, _>>()?;

		Some(Arc::new(tokio_rustls::TlsAcceptor::from(Arc::new(
			rustls::ServerConfig::builder()
				.with_no_client_auth()
				.with_single_cert(certs, key)?,
		))))
	} else {
		None
	};

	while let Ok(r) = listener.accept().context(global.ctx()).await {
		let (socket, addr) = r?;
		tracing::debug!("Accepted connection from {}", addr);

		let tls_acceptor = tls_acceptor.clone();
		let global = global.clone();

		tokio::spawn(async move {
			if let Some(tls_acceptor) = tls_acceptor {
				let Ok(Ok(socket)) = tls_acceptor.accept(socket).timeout(Duration::from_secs(5)).await else {
					return;
				};

				tracing::debug!("TLS handshake complete");
				connection::handle(global, socket, addr.ip()).await;
			} else {
				connection::handle(global, socket, addr.ip()).await;
			}
		});
	}

	Ok(())
}
