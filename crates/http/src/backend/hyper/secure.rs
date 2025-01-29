use std::{io, net::SocketAddr, sync::Arc};

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

    pub async fn run<M, B>(self, mut make_service: M, mut rustls_config: rustls::ServerConfig) -> io::Result<()>
    where
        M: tower::MakeService<SocketAddr, crate::backend::IncomingRequest, Response = http::Response<B>> + Clone,
        M::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
        M::Service: Send + Clone + 'static,
        <M::Service as tower::Service<crate::backend::IncomingRequest>>::Future: Send,
        M::MakeError: std::fmt::Debug,
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
            let (tcp_stream, addr) = match listener.accept().await {
                Ok((stream, addr)) => (stream, addr),
                Err(err) => {
                    tracing::error!("failed to accept connection: {}", err);
                    continue;
                }
            };
            tracing::info!("accepted tcp connection from {}", addr);

            let stream = match tls_acceptor.accept(tcp_stream).await {
                Ok(stream) => stream,
                Err(err) => {
                    tracing::error!("failed to accept TLS connection: {}", err);
                    continue;
                }
            };

            tracing::info!("accepted tls connection from {}", addr);
            super::handle_connection(&mut make_service, addr, stream, self.http1_enabled, self.http2_enabled).await;
        }
    }
}
