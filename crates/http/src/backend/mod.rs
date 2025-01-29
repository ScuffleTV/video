use std::{io, net::SocketAddr};

mod body;
mod error;
pub mod h3;
pub mod hyper;

pub type IncomingRequest = http::Request<body::IncomingBody>;

#[derive(Debug, Clone)]
pub enum ServerBackend {
    Insecure(hyper::insecure::InsecureBackend),
    Secure(hyper::secure::SecureBackend),
    Http3(h3::Http3Backend),
}

impl ServerBackend {
    pub fn alpn_protocols(&self) -> Vec<Vec<u8>> {
        match self {
            Self::Insecure(backend) => backend.alpn_protocols(),
            Self::Secure(backend) => backend.alpn_protocols(),
            Self::Http3(backend) => backend.alpn_protocols(),
        }
    }

    pub async fn run<M, B>(self, make_service: M, rustls_config: Option<rustls::ServerConfig>) -> io::Result<()>
    where
        M: tower::MakeService<SocketAddr, crate::backend::IncomingRequest, Response = http::Response<B>>
            + Clone
            + Send
            + 'static,
        M::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
        M::Service: Send + Clone + 'static,
        <M::Service as tower::Service<crate::backend::IncomingRequest>>::Future: Send,
        M::MakeError: std::fmt::Debug,
        M::Future: Send,
        B: http_body::Body + Send + 'static,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>> + Send,
        B::Data: Send,
    {
        match self {
            Self::Insecure(backend) => backend.run(make_service).await,
            Self::Secure(backend) => {
                backend
                    .run(
                        make_service,
                        rustls_config.ok_or(io::Error::other("tls config is required for secure backend"))?,
                    )
                    .await
            }
            Self::Http3(backend) => {
                backend
                    .run(
                        make_service,
                        rustls_config.ok_or(io::Error::other("tls config is required for http3 backend"))?,
                    )
                    .await
            }
        }
    }
}

impl From<hyper::insecure::InsecureBackend> for ServerBackend {
    fn from(backend: hyper::insecure::InsecureBackend) -> Self {
        Self::Insecure(backend)
    }
}

impl From<hyper::secure::SecureBackend> for ServerBackend {
    fn from(backend: hyper::secure::SecureBackend) -> Self {
        Self::Secure(backend)
    }
}

impl From<h3::Http3Backend> for ServerBackend {
    fn from(backend: h3::Http3Backend) -> Self {
        Self::Http3(backend)
    }
}
