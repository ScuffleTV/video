use std::fmt::{Debug, Display};
use std::net::SocketAddr;

use crate::error::Error;
use crate::service::{HttpService, HttpServiceFactory};

pub mod builder;

/// The HTTP server.
///
/// This struct is the main entry point for creating and running an HTTP server.
///
/// Create a new server using the [`ServerBuilder`](builder::ServerBuilder) struct.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct HttpServer<S> {
    ctx: scuffle_context::Context,
    service_factory: S,
    bind: SocketAddr,
    #[cfg(feature = "http1")]
    #[cfg_attr(docsrs, doc(feature = "http1"))]
    enable_http1: bool,
    #[cfg(feature = "http2")]
    #[cfg_attr(docsrs, doc(feature = "http2"))]
    enable_http2: bool,
    #[cfg(feature = "http3")]
    #[cfg_attr(docsrs, doc(feature = "http3"))]
    enable_http3: bool,
    #[cfg(feature = "tls-rustls")]
    #[cfg_attr(docsrs, doc(feature = "tls-rustls"))]
    rustls_config: Option<rustls::ServerConfig>,
}

impl<F> HttpServer<F>
where
    F: HttpServiceFactory,
{
    /// Entry point for creating a new HTTP server.
    pub fn builder() -> builder::ServerBuilder<F> {
        builder::ServerBuilder::default()
    }
}

impl<F> HttpServer<F>
where
    F: HttpServiceFactory + Clone + Send + 'static,
    F::Error: Debug + Display,
    F::Service: Clone + Send + 'static,
    <F::Service as HttpService>::Error: std::error::Error + Debug + Display + Send + Sync,
    <F::Service as HttpService>::ResBody: Send,
    <<F::Service as HttpService>::ResBody as http_body::Body>::Data: Send,
    <<F::Service as HttpService>::ResBody as http_body::Body>::Error: std::error::Error + Send + Sync,
{
    /// Run the server.
    ///
    /// This will:
    ///
    /// - Start listening on all configured interfaces for incoming connections.
    /// - Accept all incoming connections.
    /// - Handle incoming requests by passing them to the configured service factory.
    pub async fn run(self) -> Result<(), Error<F>> {
        #[allow(unused_variables)]
        #[cfg(feature = "http1")]
        let start_tcp_backend = self.enable_http1;
        #[allow(unused_variables)]
        #[cfg(feature = "http2")]
        let start_tcp_backend = self.enable_http2;
        #[cfg(all(feature = "http1", feature = "http2"))]
        let start_tcp_backend = self.enable_http1 || self.enable_http2;

        #[cfg(feature = "tls-rustls")]
        if let Some(_rustls_config) = self.rustls_config {
            #[cfg(not(any(feature = "http1", feature = "http2")))]
            let start_tcp_backend = false;

            #[allow(unused_variables)]
            let enable_http3 = false;
            #[cfg(feature = "http3")]
            let enable_http3 = self.enable_http3;

            match (start_tcp_backend, enable_http3) {
                #[cfg(feature = "http3")]
                (false, true) => {
                    use scuffle_context::ContextFutExt;

                    let backend = crate::backend::h3::Http3Backend { bind: self.bind };

                    match backend.run(self.service_factory, _rustls_config).with_context(self.ctx).await {
                        Some(res) => return res,
                        None => return Ok(()),
                    }
                }
                #[cfg(any(feature = "http1", feature = "http2"))]
                (true, false) => {
                    use scuffle_context::ContextFutExt;

                    let backend = crate::backend::hyper::secure::SecureBackend {
                        bind: self.bind,
                        #[cfg(feature = "http1")]
                        http1_enabled: self.enable_http1,
                        #[cfg(feature = "http2")]
                        http2_enabled: self.enable_http2,
                    };

                    match backend.run(self.service_factory, _rustls_config).with_context(self.ctx).await {
                        Some(res) => return res,
                        None => return Ok(()),
                    }
                }
                #[cfg(all(any(feature = "http1", feature = "http2"), feature = "http3"))]
                (true, true) => {
                    use scuffle_context::ContextFutExt;

                    let hyper = crate::backend::hyper::secure::SecureBackend {
                        bind: self.bind,
                        #[cfg(feature = "http1")]
                        http1_enabled: self.enable_http1,
                        #[cfg(feature = "http2")]
                        http2_enabled: self.enable_http2,
                    }
                    .run(self.service_factory.clone(), _rustls_config.clone());
                    let hyper = std::pin::pin!(hyper);

                    let mut http3 =
                        crate::backend::h3::Http3Backend { bind: self.bind }.run(self.service_factory, _rustls_config);
                    let http3 = std::pin::pin!(http3);

                    let res = futures::future::select(hyper, http3).with_context(self.ctx).await;
                    match res {
                        Some(futures::future::Either::Left((res, _))) => return res,
                        Some(futures::future::Either::Right((res, _))) => return res,
                        None => return Ok(()),
                    }
                }
                _ => return Ok(()),
            }
        }

        #[cfg(all(any(feature = "http1", feature = "http2"), not(feature = "tls-rustls")))]
        {
            use scuffle_context::ContextFutExt;

            #[cfg(not(any(feature = "http1", feature = "http2")))]
            let start_tcp_backend = false;

            if start_tcp_backend {
                let backend = crate::backend::hyper::insecure::InsecureBackend {
                    bind: self.bind,
                    #[cfg(feature = "http1")]
                    http1_enabled: self.enable_http1,
                    #[cfg(feature = "http2")]
                    http2_enabled: self.enable_http2,
                };

                match backend.run(self.service_factory).with_context(self.ctx).await {
                    Some(res) => return res,
                    None => return Ok(()),
                }
            }
        }

        Ok(())
    }
}
