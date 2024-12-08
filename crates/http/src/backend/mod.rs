use crate::svc::ConnectionAcceptor;

#[cfg(feature = "_quic")]
pub mod quic;
#[cfg(feature = "_tcp")]
pub mod tcp;

#[derive(derive_more::From, derive_more::Debug)]
pub enum Server {
	#[cfg(feature = "_quic")]
	Quic(quic::QuicServer),
	#[cfg(feature = "_tcp")]
	#[debug("Tcp")]
	Tcp(tcp::TcpServer),
}

#[derive(Debug, thiserror::Error)]
pub enum ServerError {
	#[cfg(feature = "_quic")]
	#[error("quic: {0}")]
	Quic(#[from] quic::QuicBackendError),
	#[cfg(feature = "_tcp")]
	#[error("tcp: {0}")]
	Tcp(#[from] tcp::TcpServerError),
}

pub trait HttpServer {
	type Error;

	/// Spawns multiple worker threads to accept connections.
	fn start<S: ConnectionAcceptor + Clone>(
		&self,
		service: S,
		workers: usize,
	) -> impl std::future::Future<Output = Result<(), Self::Error>>;

	/// Waits for the server to stop.
	fn wait(&self) -> impl std::future::Future<Output = Result<(), Self::Error>>;

	/// Shuts down the server. (blocking, waits for all connections to close)
	fn shutdown(&self) -> impl std::future::Future<Output = Result<(), Self::Error>>;

	/// Returns the address the server is bound to.
	fn local_addr(&self) -> Result<std::net::SocketAddr, Self::Error>;
}

impl HttpServer for Server {
	type Error = ServerError;

	async fn start<S: ConnectionAcceptor + Clone>(&self, service: S, workers: usize) -> Result<(), Self::Error> {
		#[cfg(not(any(feature = "_quic", feature = "_tcp")))]
		let _ = (service, workers);

		match self {
			#[cfg(feature = "_quic")]
			Server::Quic(server) => Ok(server.start(service, workers).await?),
			#[cfg(feature = "_tcp")]
			Server::Tcp(server) => Ok(server.start(service, workers).await?),
			#[cfg(not(any(feature = "_quic", feature = "_tcp")))]
			_ => unreachable!(),
		}
	}

	async fn shutdown(&self) -> Result<(), Self::Error> {
		match self {
			#[cfg(feature = "_quic")]
			Server::Quic(server) => Ok(server.shutdown().await?),
			#[cfg(feature = "_tcp")]
			Server::Tcp(server) => Ok(server.shutdown().await?),
			#[cfg(not(any(feature = "_quic", feature = "_tcp")))]
			_ => unreachable!(),
		}
	}

	fn local_addr(&self) -> Result<std::net::SocketAddr, Self::Error> {
		match self {
			#[cfg(feature = "_quic")]
			Server::Quic(server) => Ok(server.local_addr()?),
			#[cfg(feature = "_tcp")]
			Server::Tcp(server) => Ok(server.local_addr()?),
			#[cfg(not(any(feature = "_quic", feature = "_tcp")))]
			_ => unreachable!(),
		}
	}

	async fn wait(&self) -> Result<(), Self::Error> {
		match self {
			#[cfg(feature = "_quic")]
			Server::Quic(server) => Ok(server.wait().await?),
			#[cfg(feature = "_tcp")]
			Server::Tcp(server) => Ok(server.wait().await?),
			#[cfg(not(any(feature = "_quic", feature = "_tcp")))]
			_ => unreachable!(),
		}
	}
}
