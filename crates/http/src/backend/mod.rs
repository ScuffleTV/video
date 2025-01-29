use std::{
    fmt::{Debug, Display},
    net::SocketAddr,
};

use crate::error::Error;

mod body;
pub mod h3;
pub mod hyper;

pub type IncomingRequest = http::Request<body::IncomingBody>;

#[derive(Debug, Clone)]
pub enum ServerBackend {
    Insecure(hyper::insecure::InsecureBackend),
}

impl ServerBackend {
    pub fn alpn_protocols(&self) -> Vec<Vec<u8>> {
        match self {
            Self::Insecure(backend) => backend.alpn_protocols(),
        }
    }

    pub async fn run<M, B>(self, make_service: M) -> Result<(), Error<M>>
    where
        M: tower::MakeService<SocketAddr, crate::backend::IncomingRequest, Response = http::Response<B>>
            + Clone
            + Send
            + 'static,
        M::Error: std::error::Error + Display + Send + Sync + 'static,
        M::Service: Send + Clone + 'static,
        <M::Service as tower::Service<crate::backend::IncomingRequest>>::Future: Send,
        M::MakeError: Debug + Display,
        M::Future: Send,
        B: http_body::Body + Send + 'static,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>> + Send,
        B::Data: Send,
    {
        match self {
            Self::Insecure(backend) => backend.run(make_service).await,
        }
    }
}

impl From<hyper::insecure::InsecureBackend> for ServerBackend {
    fn from(backend: hyper::insecure::InsecureBackend) -> Self {
        Self::Insecure(backend)
    }
}

#[derive(Debug, Clone)]
pub enum ServerRustlsBackend {
    Insecure(hyper::insecure::InsecureBackend),
    Secure(hyper::secure::SecureBackend),
    Http3(h3::Http3Backend),
}

impl ServerRustlsBackend {
    pub fn alpn_protocols(&self) -> Vec<Vec<u8>> {
        match self {
            Self::Insecure(backend) => backend.alpn_protocols(),
            Self::Secure(backend) => backend.alpn_protocols(),
            Self::Http3(backend) => backend.alpn_protocols(),
        }
    }

    pub async fn run<M, B>(self, make_service: M, rustls_config: rustls::ServerConfig) -> Result<(), Error<M>>
    where
        M: tower::MakeService<SocketAddr, crate::backend::IncomingRequest, Response = http::Response<B>>
            + Clone
            + Send
            + 'static,
        M::Error: std::error::Error + Display + Send + Sync + 'static,
        M::Service: Send + Clone + 'static,
        <M::Service as tower::Service<crate::backend::IncomingRequest>>::Future: Send,
        M::MakeError: Debug + Display,
        M::Future: Send,
        B: http_body::Body + Send + 'static,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>> + Send,
        B::Data: Send,
    {
        match self {
            Self::Insecure(backend) => backend.run(make_service).await,
            Self::Secure(backend) => backend.run(make_service, rustls_config).await,
            Self::Http3(backend) => backend.run(make_service, rustls_config).await,
        }
    }
}

impl From<ServerBackend> for ServerRustlsBackend {
    fn from(backend: ServerBackend) -> Self {
        match backend {
            ServerBackend::Insecure(backend) => Self::Insecure(backend),
        }
    }
}

impl From<hyper::insecure::InsecureBackend> for ServerRustlsBackend {
    fn from(backend: hyper::insecure::InsecureBackend) -> Self {
        Self::Insecure(backend)
    }
}

impl From<hyper::secure::SecureBackend> for ServerRustlsBackend {
    fn from(backend: hyper::secure::SecureBackend) -> Self {
        Self::Secure(backend)
    }
}

impl From<h3::Http3Backend> for ServerRustlsBackend {
    fn from(backend: h3::Http3Backend) -> Self {
        Self::Http3(backend)
    }
}
