use std::fmt::{Debug, Display};
use std::net::SocketAddr;

use scuffle_context::ContextFutExt;

use crate::backend::h3::Http3Backend;
use crate::backend::hyper::insecure::InsecureBackend;
use crate::backend::hyper::secure::SecureBackend;
use crate::error::Error;
use crate::service::{HttpService, HttpServiceFactory};

pub mod builder;

#[derive(Debug, Clone)]
pub struct HttpServer<S> {
    ctx: scuffle_context::Context,
    service_factory: S,
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

impl<S> HttpServer<S>
where
    S: HttpServiceFactory + Clone + Send + 'static,
    S::Error: Debug + Display,
    S::Service: Clone + Send + 'static,
    <S::Service as HttpService>::Error: std::error::Error + Debug + Display + Send + Sync,
    <S::Service as HttpService>::ResBody: Send,
    <<S::Service as HttpService>::ResBody as http_body::Body>::Data: Send,
    <<S::Service as HttpService>::ResBody as http_body::Body>::Error: std::error::Error + Send + Sync,
{
    pub async fn run(self) -> Result<(), Error<S>> {
        let start_tcp_backend = self.enable_http1 || self.enable_http2;

        if let Some(rustls_config) = self.rustls_config {
            match (start_tcp_backend, self.enable_http3) {
                (false, false) => Ok(()),
                (false, true) => {
                    let backend = Http3Backend { bind: self.bind };

                    match backend.run(self.service_factory, rustls_config).with_context(self.ctx).await {
                        Some(res) => res,
                        None => Ok(()),
                    }
                }
                (true, false) => {
                    let backend = SecureBackend {
                        bind: self.bind,
                        http1_enabled: self.enable_http1,
                        http2_enabled: self.enable_http2,
                    };

                    match backend.run(self.service_factory, rustls_config).with_context(self.ctx).await {
                        Some(res) => res,
                        None => Ok(()),
                    }
                }
                (true, true) => {
                    let hyper = SecureBackend {
                        bind: self.bind,
                        http1_enabled: self.enable_http1,
                        http2_enabled: self.enable_http2,
                    }
                    .run(self.service_factory.clone(), rustls_config.clone());
                    let hyper = std::pin::pin!(hyper);

                    let mut http3 = Http3Backend { bind: self.bind }.run(self.service_factory, rustls_config);
                    let http3 = std::pin::pin!(http3);

                    let res = futures::future::select(hyper, http3).with_context(self.ctx).await;
                    match res {
                        Some(futures::future::Either::Left((res, _))) => res,
                        Some(futures::future::Either::Right((res, _))) => res,
                        None => Ok(()),
                    }
                }
            }
        } else if start_tcp_backend {
            let backend = InsecureBackend {
                bind: self.bind,
                http1_enabled: self.enable_http1,
                http2_enabled: self.enable_http2,
            };

            match backend.run(self.service_factory).with_context(self.ctx).await {
                Some(res) => res,
                None => Ok(()),
            }
        } else {
            Ok(())
        }
    }
}
