mod body;

pub(crate) use body::QuicIncomingBody;

#[cfg(feature = "quic-quinn")]
pub mod quinn;

#[cfg(feature = "http3-webtransport")]
pub mod web_transport;

use super::HttpServer;
use crate::svc::ConnectionAcceptor;

#[derive(derive_more::From, derive_more::Debug)]
pub enum QuicServer {
	#[cfg(feature = "quic-quinn")]
	#[debug("Quinn")]
	Quinn(quinn::QuinnServer),
}

#[derive(Debug, thiserror::Error)]
pub enum QuicBackendError {
	#[cfg(feature = "quic-quinn")]
	#[error("quinn: {0}")]
	Quinn(#[from] quinn::QuinnServerError),
}

impl HttpServer for QuicServer {
	type Error = QuicBackendError;

	async fn start<S: ConnectionAcceptor + Send + Sync + Clone + 'static>(
		&self,
		service: S,
		workers: usize,
	) -> Result<(), Self::Error> {
		match self {
			#[cfg(feature = "quic-quinn")]
			QuicServer::Quinn(server) => Ok(server.start(service, workers).await?),
			#[cfg(not(any(feature = "quic-quinn")))]
			_ => {
				let _ = (service, workers);
				unreachable!("impossible to construct QuicServer with no transports")
			}
		}
	}

	async fn shutdown(&self) -> Result<(), Self::Error> {
		match self {
			#[cfg(feature = "h3-quinn")]
			QuicServer::Quinn(server) => Ok(server.shutdown().await?),
			#[cfg(not(any(feature = "h3-quinn")))]
			_ => unreachable!("impossible to construct QuicServer with no transports"),
		}
	}

	fn local_addr(&self) -> Result<std::net::SocketAddr, Self::Error> {
		match self {
			#[cfg(feature = "h3-quinn")]
			QuicServer::Quinn(server) => Ok(server.local_addr()?),
			#[cfg(not(any(feature = "h3-quinn")))]
			_ => unreachable!("impossible to construct QuicServer with no transports"),
		}
	}

	async fn wait(&self) -> Result<(), Self::Error> {
		match self {
			#[cfg(feature = "h3-quinn")]
			QuicServer::Quinn(server) => Ok(server.wait().await?),
			#[cfg(not(any(feature = "h3-quinn")))]
			_ => unreachable!("impossible to construct QuicServer with no transports"),
		}
	}
}
