use std::fmt::Debug;
use std::net::SocketAddr;

use super::HttpServer;
use crate::service::HttpServiceFactory;

/// An error that can occur when building an [`HttpServer`](crate::HttpServer).
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum ServerBuilderError {
    #[error("missing bind address")]
    MissingBind,
    #[error("missing service")]
    MissingServiceFactory,
    #[error("missing rustls configuration")]
    #[cfg(feature = "tls-rustls")]
    #[cfg_attr(docsrs, doc(cfg(feature = "tls-rustls")))]
    MissingRustlsConfig,
}

/// A builder for creating an [`HttpServer`](crate::HttpServer).
///
/// Start by calling [`HttpServer::builder`].
pub struct ServerBuilder<F>
where
    F: HttpServiceFactory,
{
    ctx: Option<scuffle_context::Context>,
    bind: Option<SocketAddr>,
    service_factory: Option<F>,
    #[cfg(feature = "tls-rustls")]
    #[cfg_attr(docsrs, doc(cfg(feature = "tls-rustls")))]
    rustls_config: Option<rustls::ServerConfig>,
    #[cfg(feature = "http1")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http1")))]
    enable_http1: bool,
    #[cfg(feature = "http2")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http2")))]
    enable_http2: bool,
    #[cfg(feature = "http3")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http3")))]
    enable_http3: bool,
}

impl<F> Default for ServerBuilder<F>
where
    F: HttpServiceFactory,
{
    fn default() -> Self {
        Self {
            ctx: None,
            bind: None,
            service_factory: None,
            #[cfg(feature = "tls-rustls")]
            rustls_config: None,
            #[cfg(feature = "http1")]
            enable_http1: true,
            #[cfg(feature = "http2")]
            enable_http2: true,
            #[cfg(feature = "http3")]
            enable_http3: false,
        }
    }
}

#[cfg(feature = "tower")]
#[cfg_attr(docsrs, doc(cfg(feature = "tower")))]
impl<M> ServerBuilder<crate::service::TowerMakeServiceFactory<M, ()>>
where
    M: tower::MakeService<(), crate::IncomingRequest> + Send,
    M::Future: Send,
    M::Service: crate::service::HttpService,
{
    /// Same as calling `with_service_factory(tower_make_service_factory(tower_make_service))`.
    ///
    /// # See Also
    ///
    /// - [`with_service_factory`](ServerBuilder::with_service_factory)
    /// - [`tower_make_service_factory`](crate::service::tower_make_service_factory)
    pub fn with_tower_make_service(mut self, tower_make_service: M) -> Self {
        self.service_factory = Some(crate::service::tower_make_service_factory(tower_make_service));
        self
    }
}

#[cfg(feature = "tower")]
#[cfg_attr(docsrs, doc(cfg(feature = "tower")))]
impl<M> ServerBuilder<crate::service::TowerMakeServiceWithAddrFactory<M>>
where
    M: tower::MakeService<SocketAddr, crate::IncomingRequest> + Send,
    M::Future: Send,
    M::Service: crate::service::HttpService,
{
    /// Same as calling `with_service_factory(tower_make_service_with_addr_factory(tower_make_service))`.
    ///
    /// # See Also
    ///
    /// - [`with_service_factory`](ServerBuilder::with_service_factory)
    /// - [`tower_make_service_with_addr_factory`](crate::service::tower_make_service_with_addr_factory)
    pub fn with_tower_make_service_with_addr(mut self, tower_make_service: M) -> Self {
        self.service_factory = Some(crate::service::tower_make_service_with_addr_factory(tower_make_service));
        self
    }
}

#[cfg(feature = "tower")]
#[cfg_attr(docsrs, doc(cfg(feature = "tower")))]
impl<M, T> ServerBuilder<crate::service::TowerMakeServiceFactory<M, T>>
where
    M: tower::MakeService<T, crate::IncomingRequest> + Send,
    M::Future: Send,
    M::Service: crate::service::HttpService,
    T: Clone + Send,
{
    /// Same as calling `with_service_factory(custom_tower_make_service_factory(tower_make_service, target))`.
    ///
    /// # See Also
    ///
    /// - [`with_service_factory`](ServerBuilder::with_service_factory)
    /// - [`custom_tower_make_service_factory`](crate::service::custom_tower_make_service_factory)
    pub fn with_custom_tower_make_service(mut self, tower_make_service: M, target: T) -> Self {
        self.service_factory = Some(crate::service::custom_tower_make_service_factory(tower_make_service, target));
        self
    }
}

impl<F> ServerBuilder<F>
where
    F: HttpServiceFactory,
{
    /// Set the [`Context`](scuffle_context::Context) for the server.
    ///
    /// The server will terminate when the context is canceled.
    ///
    /// This is optional and defaults to the global context.
    ///
    /// # See Also
    ///
    /// - [`scuffle_context`]
    pub fn with_ctx(mut self, ctx: scuffle_context::Context) -> Self {
        self.ctx = Some(ctx);
        self
    }

    /// Set the bind address for the server.
    ///
    /// This is required to build the server.
    ///
    /// Note: Using `[::]` as the IP part of the address will bind to all available interfaces.
    pub fn bind(mut self, bind: SocketAddr) -> Self {
        self.bind = Some(bind);
        self
    }

    /// Set the service factory for the server.
    ///
    /// This is required to build the server.
    ///
    /// See [`service`](crate::service) for a list of built-in service factories.
    /// You can also implement your own service factory by implementing the [`HttpServiceFactory`](crate::service::HttpServiceFactory) trait.
    pub fn with_service_factory(mut self, service_factory: F) -> Self {
        self.service_factory = Some(service_factory);
        self
    }

    /// Enable or disable HTTP/1 support based on the given bool.
    ///
    /// Enabled by default.
    #[cfg(feature = "http1")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http1")))]
    pub fn http1(mut self, enable: bool) -> Self {
        self.enable_http1 = enable;
        self
    }

    /// Enable HTTP/1 support.
    ///
    /// Enabled by default.
    #[cfg(feature = "http1")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http1")))]
    pub fn enable_http1(self) -> Self {
        self.http1(true)
    }

    /// Disable HTTP/1 support.
    ///
    /// Enabled by default.
    #[cfg(feature = "http1")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http1")))]
    pub fn disable_http1(self) -> Self {
        self.http1(false)
    }

    /// Enable or disable HTTP/2 support based on the given bool.
    ///
    /// Enabled by default.
    #[cfg(feature = "http2")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http2")))]
    pub fn http2(mut self, enable: bool) -> Self {
        self.enable_http2 = enable;
        self
    }

    /// Enable HTTP/2 support.
    ///
    /// Enabled by default.
    #[cfg(feature = "http2")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http2")))]
    pub fn enable_http2(self) -> Self {
        self.http2(true)
    }

    /// Disable HTTP/2 support.
    ///
    /// Enabled by default.
    #[cfg(feature = "http2")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http2")))]
    pub fn disable_http2(self) -> Self {
        self.http2(false)
    }

    /// Enable or disable HTTP/3 support based on the given bool.
    ///
    /// Disabled by default.
    #[cfg(feature = "http3")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http3")))]
    pub fn http3(mut self, enable: bool) -> Self {
        self.enable_http3 = enable;
        self
    }

    /// Enable HTTP/3 support.
    ///
    /// Disabled by default.
    #[cfg(feature = "http3")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http3")))]
    pub fn enable_http3(self) -> Self {
        self.http3(true)
    }

    /// Disable HTTP/3 support.
    ///
    /// Disabled by default.
    #[cfg(feature = "http3")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http3")))]
    pub fn disable_http3(self) -> Self {
        self.http3(false)
    }

    /// Set the rustls configuration for the server.
    ///
    /// This enables TLS support for the server.
    ///
    /// Required for HTTP/3 support.
    #[cfg(feature = "tls-rustls")]
    #[cfg_attr(docsrs, doc(cfg(feature = "tls-rustls")))]
    pub fn with_rustls(mut self, config: rustls::ServerConfig) -> Self {
        self.rustls_config = Some(config);
        self
    }

    /// Build the [`HttpServer`](crate::HttpServer) from this builder.
    ///
    /// Make sure to set the bind address and service factory before calling this method.
    /// If HTTP/3 support is enabled, the rustls configuration must be set as well.
    pub fn build(self) -> Result<HttpServer<F>, ServerBuilderError> {
        let this = self;
        #[cfg(feature = "tls-rustls")]
        let mut this = this;

        // https://www.iana.org/assignments/tls-extensiontype-values/tls-extensiontype-values.xhtml#alpn-protocol-ids
        #[cfg(feature = "tls-rustls")]
        if let Some(rustlsconfig) = &mut this.rustls_config {
            rustlsconfig.alpn_protocols.clear();

            #[cfg(feature = "http1")]
            if this.enable_http1 {
                rustlsconfig.alpn_protocols.push(b"http/1.0".to_vec());
                rustlsconfig.alpn_protocols.push(b"http/1.1".to_vec());
            }

            #[cfg(feature = "http2")]
            if this.enable_http2 {
                rustlsconfig.alpn_protocols.push(b"h2".to_vec());
                rustlsconfig.alpn_protocols.push(b"h2c".to_vec());
            }

            #[cfg(feature = "http3")]
            if this.enable_http3 {
                rustlsconfig.alpn_protocols.push(b"h3".to_vec());
            }
        }

        #[cfg(all(feature = "tls-rustls", feature = "http3"))]
        if this.rustls_config.is_none() && this.enable_http3 {
            // HTTP/3 doesn't work without TLS
            return Err(ServerBuilderError::MissingRustlsConfig);
        }

        Ok(HttpServer {
            ctx: this.ctx.unwrap_or_else(scuffle_context::Context::global),
            service_factory: this.service_factory.ok_or(ServerBuilderError::MissingServiceFactory)?,
            bind: this.bind.ok_or(ServerBuilderError::MissingBind)?,
            #[cfg(feature = "tls-rustls")]
            rustls_config: this.rustls_config,
            #[cfg(feature = "http1")]
            enable_http1: this.enable_http1,
            #[cfg(feature = "http2")]
            enable_http2: this.enable_http2,
            #[cfg(feature = "http3")]
            enable_http3: this.enable_http3,
        })
    }
}

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use std::convert::Infallible;

    use super::ServerBuilder;
    use crate::server::builder::ServerBuilderError;
    use crate::service::{fn_http_service, service_clone_factory};

    fn get_available_addr() -> std::io::Result<std::net::SocketAddr> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        listener.local_addr()
    }

    const RESPONSE_TEXT: &str = "Hello, world!";

    #[test]
    fn missing_bind() {
        let builder = ServerBuilder::default().with_service_factory(service_clone_factory(fn_http_service(|_| async {
            Ok::<_, Infallible>(http::Response::new(RESPONSE_TEXT.to_string()))
        })));

        assert_eq!(builder.build().unwrap_err(), ServerBuilderError::MissingBind);
    }

    #[test]
    fn missing_rustls() {
        let builder = ServerBuilder::default()
            .with_service_factory(service_clone_factory(fn_http_service(|_| async {
                Ok::<_, Infallible>(http::Response::new(RESPONSE_TEXT.to_string()))
            })))
            .enable_http3();

        assert_eq!(builder.build().unwrap_err(), ServerBuilderError::MissingRustlsConfig);
    }

    #[tokio::test]
    async fn simple() {
        let mut builder = ServerBuilder::default().with_service_factory(service_clone_factory(fn_http_service(|_| async {
            Ok::<_, Infallible>(http::Response::new("".to_string()))
        })));

        assert!(builder.ctx.is_none());
        assert!(builder.bind.is_none());
        assert!(builder.service_factory.is_some());
        assert!(builder.rustls_config.is_none());
        assert!(builder.enable_http1);
        assert!(builder.enable_http2);
        assert!(!builder.enable_http3);

        builder = builder.disable_http1();
        assert!(!builder.enable_http1);
        builder = builder.http1(false);
        assert!(!builder.enable_http1);
        builder = builder.enable_http1();
        assert!(builder.enable_http1);

        builder = builder.disable_http2();
        assert!(!builder.enable_http2);
        builder = builder.http2(false);
        assert!(!builder.enable_http2);
        builder = builder.enable_http2();
        assert!(builder.enable_http2);

        builder = builder.enable_http3();
        assert!(builder.enable_http3);
        builder = builder.http3(false);
        assert!(!builder.enable_http3);
        builder = builder.disable_http3();
        assert!(!builder.enable_http3);

        let addr = get_available_addr().expect("failed to get available address");
        builder = builder.bind(addr);
        assert!(builder.bind.is_some());

        let (ctx, _) = scuffle_context::Context::new();

        builder = builder.with_ctx(ctx);
        assert!(builder.ctx.is_some());

        let server = builder.build().unwrap();
        assert!(server.rustls_config.is_none());
        assert!(server.enable_http1);
        assert!(server.enable_http2);
        assert!(!server.enable_http3);
    }
}
