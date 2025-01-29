use std::{
    fmt::{Debug, Display},
    net::SocketAddr,
};

use futures::{stream::FuturesUnordered, StreamExt};
use itertools::Itertools;

use crate::{
    backend::{ServerBackend, ServerRustlsBackend},
    error::Error,
};

#[derive(Debug, Clone)]
pub struct Server {
    backends: Vec<ServerBackend>,
}

impl Server {
    pub fn new() -> Self {
        Self { backends: Vec::new() }
    }

    pub fn with_backend(mut self, backend: impl Into<ServerBackend>) -> Self {
        self.backends.push(backend.into());
        self
    }

    pub fn with_rustls_config(self, rustls_config: rustls::ServerConfig) -> ServerWithRustls {
        ServerWithRustls {
            backends: self.backends.into_iter().map(Into::into).collect(),
            rustls_config,
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
        let mut futures: FuturesUnordered<_> = self.backends.into_iter().map(|b| b.run(make_service.clone())).collect();

        while let Some(res) = futures.next().await {
            res?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ServerWithRustls {
    backends: Vec<ServerRustlsBackend>,
    rustls_config: rustls::ServerConfig,
}

impl ServerWithRustls {
    pub fn with_backend(mut self, backend: impl Into<ServerRustlsBackend>) -> Self {
        self.backends.push(backend.into());
        self
    }

    pub async fn run<M, B>(mut self, make_service: M) -> Result<(), Error<M>>
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
        let alpns = self
            .backends
            .iter()
            .flat_map(|b| b.alpn_protocols())
            .dedup()
            .collect::<Vec<_>>();
        self.rustls_config.alpn_protocols = alpns;

        let mut futures: FuturesUnordered<_> = self
            .backends
            .into_iter()
            .map(|b| b.run(make_service.clone(), self.rustls_config.clone()))
            .collect();

        while let Some(res) = futures.next().await {
            res?;
        }

        Ok(())
    }
}
