use std::fmt::{Debug, Display};
use std::net::SocketAddr;

use crate::error::Error;

#[derive(Debug, Clone)]
pub struct InsecureBackend {
    pub bind: SocketAddr,
    pub http1_enabled: bool,
    pub http2_enabled: bool,
}

impl Default for InsecureBackend {
    fn default() -> Self {
        Self {
            bind: "[::]:80".parse().unwrap(),
            http1_enabled: true,
            http2_enabled: true,
        }
    }
}

impl InsecureBackend {
    pub fn alpn_protocols(&self) -> Vec<Vec<u8>> {
        let mut protocols = Vec::new();

        if self.http1_enabled {
            // HTTP/1.0 and HTTP/1.1
            protocols.push(b"http/1.0".to_vec());
            protocols.push(b"http/1.1".to_vec());
        }

        if self.http2_enabled {
            // HTTP/2 over cleartext TCP
            protocols.push(b"h2c".to_vec());
        }

        protocols
    }

    pub async fn run<M, D>(self, mut make_service: M) -> Result<(), Error<M>>
    where
        M: tower::MakeService<SocketAddr, crate::backend::IncomingRequest, Response = http::Response<D>> + Clone,
        M::Error: std::error::Error + Display + Send + Sync + 'static,
        M::Service: Send + Clone + 'static,
        <M::Service as tower::Service<crate::backend::IncomingRequest>>::Future: Send,
        M::MakeError: Debug + Display,
        M::Future: Send,
        D: http_body::Body + Send + 'static,
        D::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
        D::Data: Send,
    {
        tracing::info!("starting server");

        let listener = tokio::net::TcpListener::bind(self.bind).await?;

        loop {
            let res: Result<_, Error<M>> = async {
                let (tcp_stream, addr) = listener.accept().await?;
                super::handle_connection(&mut make_service, addr, tcp_stream, self.http1_enabled, self.http2_enabled)
                    .await?;

                Ok(())
            }
            .await;

            if let Err(e) = res {
                tracing::warn!("error: {}", e);
            }
        }
    }
}
