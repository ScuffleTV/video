use std::fmt::{Debug, Display};
use std::net::SocketAddr;
use std::sync::Arc;

use crate::error::Error;
use crate::service::{HttpService, HttpServiceFactory};

#[derive(Debug, Clone)]
pub struct SecureBackend {
    pub bind: SocketAddr,
    pub http1_enabled: bool,
    pub http2_enabled: bool,
}

impl SecureBackend {
    pub async fn run<S>(self, mut service_factory: S, mut rustls_config: rustls::ServerConfig) -> Result<(), Error<S>>
    where
        S: HttpServiceFactory,
        S::Error: Debug + Display,
        S::Service: Clone + Send + 'static,
        <S::Service as HttpService>::Error: std::error::Error + Debug + Display + Send + Sync,
        <S::Service as HttpService>::ResBody: Send,
        <<S::Service as HttpService>::ResBody as http_body::Body>::Data: Send,
        <<S::Service as HttpService>::ResBody as http_body::Body>::Error: std::error::Error + Send + Sync,
    {
        tracing::debug!("starting server");

        // reset it back to 0 because everything explodes if it's not
        // https://github.com/hyperium/hyper/issues/3841
        rustls_config.max_early_data_size = 0;

        let listener = tokio::net::TcpListener::bind(self.bind).await?;
        let tls_acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(rustls_config));

        loop {
            let res: Result<_, Error<S>> = async {
                let (tcp_stream, addr) = listener.accept().await?;
                let stream = tls_acceptor.accept(tcp_stream).await?;
                super::handle_connection(&mut service_factory, addr, stream, self.http1_enabled, self.http2_enabled).await?;

                Ok(())
            }
            .await;

            if let Err(err) = res {
                tracing::warn!("error: {}", err);
            }
        }
    }
}
