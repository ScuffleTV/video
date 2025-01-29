use std::{io, net::SocketAddr};

use crate::{backend, config::Config};

#[derive(Debug, Clone)]
pub struct Server<M> {
    pub config: Config,
    pub tls_config: Option<rustls::ServerConfig>,
    pub make_service: M,
}

impl<M> Server<M> {
    pub fn tls_config(&self) -> Option<rustls::ServerConfig> {
        let mut tls_config = self.tls_config.clone()?;
        tls_config.alpn_protocols = self.config.alpn_protocols();
        Some(tls_config)
    }
}

impl<M, B> Server<M>
where
    M: tower::MakeService<
            SocketAddr,
            crate::backend::IncomingRequest,
            Response = http::Response<B>,
        > + Clone
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
    pub async fn run(self) -> io::Result<()> {
        // let mut insecure = std::pin::pin!(backend::hyper::insecure::server(&self).fuse());
        // let mut secure = std::pin::pin!(backend::hyper::secure::server(&self).fuse());
        // let mut http3 = std::pin::pin!(backend::h3::server(&self).fuse());
        //
        // let res = futures::select! {
        //     res = insecure => res,
        //     res = secure => res,
        //     res = http3 => res,
        // };

        let insecure = backend::hyper::insecure::server(&self);
        let secure = backend::hyper::secure::server(&self);
        let http3 = backend::h3::server(&self);

        let (res_insecure, res_secure, res_http3) = futures::join! {
            insecure,
            secure,
            http3,
        };

        res_insecure?;
        res_secure?;
        res_http3?;

        Ok(())
    }
}
