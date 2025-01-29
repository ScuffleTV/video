use std::{io, net::SocketAddr};

use futures::{stream::FuturesUnordered, StreamExt};
use itertools::Itertools;

use crate::backend::ServerBackend;

#[derive(Debug, Clone)]
pub struct Server {
    backends: Vec<ServerBackend>,
    rustls_config: Option<rustls::ServerConfig>,
}

impl Server {
    pub fn new() -> Self {
        Self {
            backends: Vec::new(),
            rustls_config: None,
        }
    }

    pub fn with_backend(mut self, backend: impl Into<ServerBackend>) -> Self {
        self.backends.push(backend.into());
        self
    }

    pub fn with_rustls_config(mut self, rustls_config: rustls::ServerConfig) -> Self {
        self.rustls_config = Some(rustls_config);
        self
    }

    pub async fn run<M, B>(mut self, make_service: M) -> io::Result<()>
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
        if let Some(rustls_config) = &mut self.rustls_config {
            let alpns = self
                .backends
                .iter()
                .flat_map(|b| b.alpn_protocols())
                .dedup()
                .collect::<Vec<_>>();

            rustls_config.alpn_protocols = alpns;
        }

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
