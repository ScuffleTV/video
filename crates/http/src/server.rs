use std::fmt::{Debug, Display};
use std::net::SocketAddr;

use crate::error::Error;
use crate::service::{HttpService, HttpServiceFactory};

/// The HTTP server.
///
/// This struct is the main entry point for creating and running an HTTP server.
///
/// Start creating a new server by calling [`HttpServer::builder`].
#[derive(Debug, Clone, bon::Builder)]
#[builder(state_mod(vis = "pub(crate)"))]
#[allow(dead_code)]
pub struct HttpServer<F> {
    #[builder(default)]
    ctx: scuffle_context::Context,
    service_factory: F,
    bind: SocketAddr,
    #[builder(default = true)]
    #[cfg(feature = "http1")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http1")))]
    enable_http1: bool,
    #[builder(default = true)]
    #[cfg(feature = "http2")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http2")))]
    enable_http2: bool,
    #[builder(default = false, setters(vis = "", name = enable_http3_internal))]
    #[cfg(feature = "http3")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http3")))]
    enable_http3: bool,
    #[cfg(feature = "tls-rustls")]
    #[cfg_attr(docsrs, doc(cfg(feature = "tls-rustls")))]
    rustls_config: Option<rustls::ServerConfig>,
}

#[cfg(feature = "http3")]
#[cfg_attr(docsrs, doc(cfg(feature = "http3")))]
impl<F, S> HttpServerBuilder<F, S>
where
    S: http_server_builder::State,
    S::EnableHttp3: http_server_builder::IsUnset,
    S::RustlsConfig: http_server_builder::IsSet,
{
    pub fn enable_http3(self, enable_http3: bool) -> HttpServerBuilder<F, http_server_builder::SetEnableHttp3<S>> {
        self.enable_http3_internal(enable_http3)
    }
}

#[cfg(feature = "tower")]
#[cfg_attr(docsrs, doc(cfg(feature = "tower")))]
impl<M, S> HttpServerBuilder<crate::service::TowerMakeServiceFactory<M, ()>, S>
where
    M: tower::MakeService<(), crate::IncomingRequest> + Send,
    M::Future: Send,
    M::Service: crate::service::HttpService,
    S: http_server_builder::State,
    S::ServiceFactory: http_server_builder::IsUnset,
{
    /// Same as calling `service_factory(tower_make_service_factory(tower_make_service))`.
    ///
    /// # See Also
    ///
    /// - [`service_factory`](HttpServerBuilder::service_factory)
    /// - [`tower_make_service_factory`](crate::service::tower_make_service_factory)
    pub fn tower_make_service_factory(
        self,
        tower_make_service: M,
    ) -> HttpServerBuilder<crate::service::TowerMakeServiceFactory<M, ()>, http_server_builder::SetServiceFactory<S>> {
        self.service_factory(crate::service::tower_make_service_factory(tower_make_service))
    }
}

#[cfg(feature = "tower")]
#[cfg_attr(docsrs, doc(cfg(feature = "tower")))]
impl<M, S> HttpServerBuilder<crate::service::TowerMakeServiceWithAddrFactory<M>, S>
where
    M: tower::MakeService<SocketAddr, crate::IncomingRequest> + Send,
    M::Future: Send,
    M::Service: crate::service::HttpService,
    S: http_server_builder::State,
    S::ServiceFactory: http_server_builder::IsUnset,
{
    /// Same as calling `service_factory(tower_make_service_with_addr_factory(tower_make_service))`.
    ///
    /// # See Also
    ///
    /// - [`service_factory`](HttpServerBuilder::service_factory)
    /// - [`tower_make_service_with_addr_factory`](crate::service::tower_make_service_with_addr_factory)
    pub fn tower_make_service_with_addr(
        self,
        tower_make_service: M,
    ) -> HttpServerBuilder<crate::service::TowerMakeServiceWithAddrFactory<M>, http_server_builder::SetServiceFactory<S>>
    {
        self.service_factory(crate::service::tower_make_service_with_addr_factory(tower_make_service))
    }
}

#[cfg(feature = "tower")]
#[cfg_attr(docsrs, doc(cfg(feature = "tower")))]
impl<M, T, S> HttpServerBuilder<crate::service::TowerMakeServiceFactory<M, T>, S>
where
    M: tower::MakeService<T, crate::IncomingRequest> + Send,
    M::Future: Send,
    M::Service: crate::service::HttpService,
    T: Clone + Send,
    S: http_server_builder::State,
    S::ServiceFactory: http_server_builder::IsUnset,
{
    /// Same as calling `service_factory(custom_tower_make_service_factory(tower_make_service, target))`.
    ///
    /// # See Also
    ///
    /// - [`service_factory`](HttpServerBuilder::service_factory)
    /// - [`custom_tower_make_service_factory`](crate::service::custom_tower_make_service_factory)
    pub fn custom_tower_make_service_factory(
        self,
        tower_make_service: M,
        target: T,
    ) -> HttpServerBuilder<crate::service::TowerMakeServiceFactory<M, T>, http_server_builder::SetServiceFactory<S>> {
        self.service_factory(crate::service::custom_tower_make_service_factory(tower_make_service, target))
    }
}

impl<F> HttpServer<F>
where
    F: HttpServiceFactory + Clone + Send + 'static,
    F::Error: Debug + Display + Send,
    F::Service: Clone + Send + 'static,
    <F::Service as HttpService>::Error: std::error::Error + Debug + Send + Sync,
    <F::Service as HttpService>::ResBody: Send,
    <<F::Service as HttpService>::ResBody as http_body::Body>::Data: Send,
    <<F::Service as HttpService>::ResBody as http_body::Body>::Error: std::error::Error + Send + Sync,
{
    #[cfg(feature = "tls-rustls")]
    fn set_alpn_protocols(&mut self) {
        let Some(rustls_config) = &mut self.rustls_config else {
            return;
        };

        // https://www.iana.org/assignments/tls-extensiontype-values/tls-extensiontype-values.xhtml#alpn-protocol-ids
        if rustls_config.alpn_protocols.is_empty() {
            #[cfg(feature = "http1")]
            if self.enable_http1 {
                rustls_config.alpn_protocols.push(b"http/1.0".to_vec());
                rustls_config.alpn_protocols.push(b"http/1.1".to_vec());
            }

            #[cfg(feature = "http2")]
            if self.enable_http2 {
                rustls_config.alpn_protocols.push(b"h2".to_vec());
                rustls_config.alpn_protocols.push(b"h2c".to_vec());
            }

            #[cfg(feature = "http3")]
            if self.enable_http3 {
                rustls_config.alpn_protocols.push(b"h3".to_vec());
            }
        }
    }

    /// Run the server.
    ///
    /// This will:
    ///
    /// - Start listening on all configured interfaces for incoming connections.
    /// - Accept all incoming connections.
    /// - Handle incoming requests by passing them to the configured service factory.
    pub async fn run(#[allow(unused_mut)] mut self) -> Result<(), Error<F>> {
        #[cfg(feature = "tls-rustls")]
        self.set_alpn_protocols();

        #[cfg(all(not(any(feature = "http1", feature = "http2")), feature = "tls-rustls"))]
        let start_tcp_backend = false;
        #[cfg(all(feature = "http1", not(feature = "http2")))]
        let start_tcp_backend = self.enable_http1;
        #[cfg(all(not(feature = "http1"), feature = "http2"))]
        let start_tcp_backend = self.enable_http2;
        #[cfg(all(feature = "http1", feature = "http2"))]
        let start_tcp_backend = self.enable_http1 || self.enable_http2;

        #[cfg(feature = "tls-rustls")]
        if let Some(_rustls_config) = self.rustls_config {
            #[cfg(not(feature = "http3"))]
            let enable_http3 = false;
            #[cfg(feature = "http3")]
            let enable_http3 = self.enable_http3;

            match (start_tcp_backend, enable_http3) {
                #[cfg(feature = "http3")]
                (false, true) => {
                    use scuffle_context::ContextFutExt;

                    let backend = crate::backend::h3::Http3Backend {
                        ctx: self.ctx.clone(),
                        bind: self.bind,
                    };

                    match backend.run(self.service_factory, _rustls_config).with_context(self.ctx).await {
                        Some(res) => return res,
                        None => return Ok(()),
                    }
                }
                #[cfg(any(feature = "http1", feature = "http2"))]
                (true, false) => {
                    use scuffle_context::ContextFutExt;

                    let backend = crate::backend::hyper::secure::SecureBackend {
                        ctx: self.ctx.clone(),
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
                    let hyper = crate::backend::hyper::secure::SecureBackend {
                        ctx: self.ctx.clone(),
                        bind: self.bind,
                        #[cfg(feature = "http1")]
                        http1_enabled: self.enable_http1,
                        #[cfg(feature = "http2")]
                        http2_enabled: self.enable_http2,
                    }
                    .run(self.service_factory.clone(), _rustls_config.clone());
                    let hyper = std::pin::pin!(hyper);

                    let mut http3 = crate::backend::h3::Http3Backend {
                        ctx: self.ctx.clone(),
                        bind: self.bind,
                    }
                    .run(self.service_factory, _rustls_config);
                    let http3 = std::pin::pin!(http3);

                    let res = futures::future::select(hyper, http3).await;
                    match res {
                        futures::future::Either::Left((res, _)) => return res,
                        futures::future::Either::Right((res, _)) => return res,
                    }
                }
                _ => return Ok(()),
            }

            // This line must be unreachable
        }

        // At this point we know that we are not using TLS either
        // - because the feature is disabled
        // - or because it's enabled but the config is None.

        #[cfg(any(feature = "http1", feature = "http2"))]
        if start_tcp_backend {
            use scuffle_context::ContextFutExt;

            let backend = crate::backend::hyper::insecure::InsecureBackend {
                ctx: self.ctx.clone(),
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

        Ok(())
    }
}
