use crate::svc::ConnectionAcceptor;

#[cfg(feature = "http3")]
pub mod quic;
#[cfg(any(feature = "http1", feature = "http2"))]
pub mod tcp;

#[derive(derive_more::From, derive_more::Debug)]
pub enum Server {
    #[cfg(feature = "http3")]
    Quic(quic::QuicServer),
    #[cfg(any(feature = "http1", feature = "http2"))]
    #[debug("Tcp")]
    Tcp(tcp::TcpServer),
}

#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[cfg(feature = "http3")]
    #[error("quic: {0}")]
    Quic(#[from] quic::QuicBackendError),
    #[cfg(any(feature = "http1", feature = "http2"))]
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
        #[cfg(not(any(feature = "http1", feature = "http2", feature = "http3")))]
        let _ = (service, workers);

        match self {
            #[cfg(feature = "http3")]
            Server::Quic(server) => Ok(server.start(service, workers).await?),
            #[cfg(any(feature = "http1", feature = "http2"))]
            Server::Tcp(server) => Ok(server.start(service, workers).await?),
            #[cfg(not(any(feature = "http3", feature = "http1", feature = "http2")))]
            _ => unreachable!(),
        }
    }

    async fn shutdown(&self) -> Result<(), Self::Error> {
        match self {
            #[cfg(feature = "http3")]
            Server::Quic(server) => Ok(server.shutdown().await?),
            #[cfg(any(feature = "http1", feature = "http2"))]
            Server::Tcp(server) => Ok(server.shutdown().await?),
            #[cfg(not(any(feature = "http3", feature = "http1", feature = "http2")))]
            _ => unreachable!(),
        }
    }

    fn local_addr(&self) -> Result<std::net::SocketAddr, Self::Error> {
        match self {
            #[cfg(feature = "http3")]
            Server::Quic(server) => Ok(server.local_addr()?),
            #[cfg(any(feature = "http1", feature = "http2"))]
            Server::Tcp(server) => Ok(server.local_addr()?),
            #[cfg(not(any(feature = "http3", feature = "http1", feature = "http2")))]
            _ => unreachable!(),
        }
    }

    async fn wait(&self) -> Result<(), Self::Error> {
        match self {
            #[cfg(feature = "http3")]
            Server::Quic(server) => Ok(server.wait().await?),
            #[cfg(any(feature = "http1", feature = "http2"))]
            Server::Tcp(server) => Ok(server.wait().await?),
            #[cfg(not(any(feature = "http3", feature = "http1", feature = "http2")))]
            _ => unreachable!(),
        }
    }
}
