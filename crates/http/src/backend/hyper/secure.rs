use std::{
    fmt::{Debug, Display},
    net::SocketAddr,
    sync::Arc,
};

use crate::error::Error;

#[derive(Debug, Clone)]
pub struct SecureBackend {
    pub bind: SocketAddr,
    pub http1_enabled: bool,
    pub http2_enabled: bool,
}

impl Default for SecureBackend {
    fn default() -> Self {
        Self {
            bind: "[::]:443".parse().unwrap(),
            http1_enabled: true,
            http2_enabled: true,
        }
    }
}

impl SecureBackend {
    pub fn alpn_protocols(&self) -> Vec<Vec<u8>> {
        let mut protocols = Vec::new();

        if self.http1_enabled {
            // HTTP/1.0 and HTTP/1.1
            protocols.push(b"http/1.0".to_vec());
            protocols.push(b"http/1.1".to_vec());
        }

        if self.http2_enabled {
            // HTTP/2 over TLS
            protocols.push(b"h2".to_vec());
        }

        protocols
    }

    pub async fn run<M, B>(
        self,
        mut make_service: M,
        mut rustls_config: rustls::ServerConfig,
    ) -> Result<(), Error<M>>
    where
        M: tower::MakeService<SocketAddr, crate::backend::IncomingRequest, Response = http::Response<B>> + Clone,
        M::Error: std::error::Error + Display + Send + Sync + 'static,
        M::Service: Send + Clone + 'static,
        <M::Service as tower::Service<crate::backend::IncomingRequest>>::Future: Send,
        M::MakeError: Debug + Display,
        M::Future: Send,
        B: http_body::Body + Send + 'static,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
        B::Data: Send,
    {
        tracing::info!("starting server");

        // reset it back to 0 because everything explodes if it's not
        // https://github.com/hyperium/hyper/issues/3841
        rustls_config.max_early_data_size = 0;

        let listener = tokio::net::TcpListener::bind(self.bind).await?;
        let tls_acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(rustls_config));

        loop {
            let res: Result<_, Error<M>> = async {
                let (tcp_stream, addr) = listener.accept().await?;
                let stream = tls_acceptor.accept(tcp_stream).await?;
                super::handle_connection(&mut make_service, addr, stream, self.http1_enabled, self.http2_enabled).await?;

                Ok(())
            }
            .await;

            if let Err(err) = res {
                tracing::warn!("error: {}", err);
            }
        }
    }
}
