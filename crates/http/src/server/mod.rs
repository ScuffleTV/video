use std::fmt::{Debug, Display};
use std::net::SocketAddr;

use scuffle_context::ContextFutExt;

use crate::backend::h3::Http3Backend;
use crate::backend::hyper::insecure::InsecureBackend;
use crate::backend::hyper::secure::SecureBackend;
use crate::error::Error;

mod builder;

#[derive(Debug, Clone)]
pub struct HttpServer<M> {
    ctx: scuffle_context::Context,
    service: M,
    bind: SocketAddr,
    enable_http1: bool,
    enable_http2: bool,
    enable_http3: bool,
    rustls_config: Option<rustls::ServerConfig>,
}

impl<M> HttpServer<M> {
    pub fn builder() -> builder::ServerBuilder<M> {
        builder::ServerBuilder::default()
    }
}

impl<M, B> HttpServer<M>
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
    pub async fn run(self) -> Result<(), Error<M>> {
        if let Some(rustls_config) = self.rustls_config {
            if self.enable_http3 {
                let hyper = SecureBackend {
                    bind: self.bind,
                    http1_enabled: self.enable_http1,
                    http2_enabled: self.enable_http2,
                }
                .run(self.service.clone(), rustls_config.clone());
                let hyper = std::pin::pin!(hyper);

                let mut http3 = Http3Backend { bind: self.bind }.run(self.service, rustls_config);
                let http3 = std::pin::pin!(http3);

                let res = futures::future::select(hyper, http3).with_context(self.ctx).await;
                match res {
                    Some(futures::future::Either::Left((res, _))) => res,
                    Some(futures::future::Either::Right((res, _))) => res,
                    None => Ok(()),
                }
            } else if self.enable_http1 || self.enable_http2 {
                let backend = SecureBackend {
                    bind: self.bind,
                    http1_enabled: self.enable_http1,
                    http2_enabled: self.enable_http2,
                };

                match backend.run(self.service, rustls_config).with_context(self.ctx).await {
                    Some(res) => res,
                    None => Ok(()),
                }
            } else {
                Ok(())
            }
        } else if self.enable_http1 || self.enable_http2 {
            let backend = InsecureBackend {
                bind: self.bind,
                http1_enabled: self.enable_http1,
                http2_enabled: self.enable_http2,
            };

            match backend.run(self.service).with_context(self.ctx).await {
                Some(res) => res,
                None => Ok(()),
            }
        } else {
            Ok(())
        }
    }
}
