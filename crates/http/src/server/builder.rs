use std::fmt::Debug;
use std::net::SocketAddr;

use super::HttpServer;
use crate::service::{
    custom_tower_make_service_factory, tower_make_service_factory, tower_make_service_with_addr_factory, HttpService,
    HttpServiceFactory, TowerMakeServiceFactory, TowerMakeServiceWithAddrFactory,
};
use crate::IncomingRequest;

/// An error that can occur when building an [`HttpServer`](crate::HttpServer).
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum ServerBuilderError {
    #[error("missing bind address")]
    MissingBind,
    #[error("missing service")]
    MissingServiceFactory,
    #[error("missing rustls configuration")]
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
    rustls_config: Option<rustls::ServerConfig>,
    enable_http1: bool,
    enable_http2: bool,
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
            rustls_config: None,
            enable_http1: true,
            enable_http2: true,
            enable_http3: false,
        }
    }
}

impl<M> ServerBuilder<TowerMakeServiceFactory<M, ()>>
where
    M: tower::MakeService<(), IncomingRequest> + Send,
    M::Future: Send,
    M::Service: HttpService,
{
    /// Same as calling `with_service_factory(tower_make_service_factory(tower_make_service))`.
    ///
    /// # See Also
    ///
    /// - [`with_service_factory`](ServerBuilder::with_service_factory)
    /// - [`tower_make_service_factory`]
    pub fn with_tower_make_service(mut self, tower_make_service: M) -> Self {
        self.service_factory = Some(tower_make_service_factory(tower_make_service));
        self
    }
}

impl<M> ServerBuilder<TowerMakeServiceWithAddrFactory<M>>
where
    M: tower::MakeService<SocketAddr, IncomingRequest> + Send,
    M::Future: Send,
    M::Service: HttpService,
{
    /// Same as calling `with_service_factory(tower_make_service_with_addr_factory(tower_make_service))`.
    ///
    /// # See Also
    ///
    /// - [`with_service_factory`](ServerBuilder::with_service_factory)
    /// - [`tower_make_service_with_addr_factory`]
    pub fn with_tower_make_service_with_addr(mut self, tower_make_service: M) -> Self {
        self.service_factory = Some(tower_make_service_with_addr_factory(tower_make_service));
        self
    }
}

impl<M, T> ServerBuilder<TowerMakeServiceFactory<M, T>>
where
    M: tower::MakeService<T, IncomingRequest> + Send,
    M::Future: Send,
    M::Service: HttpService,
    T: Clone + Send,
{
    /// Same as calling `with_service_factory(custom_tower_make_service_factory(tower_make_service, target))`.
    ///
    /// # See Also
    ///
    /// - [`with_service_factory`](ServerBuilder::with_service_factory)
    /// - [`custom_tower_make_service_factory`]
    pub fn with_custom_tower_make_service(mut self, tower_make_service: M, target: T) -> Self {
        self.service_factory = Some(custom_tower_make_service_factory(tower_make_service, target));
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
    pub fn http1(mut self, enable: bool) -> Self {
        self.enable_http1 = enable;
        self
    }

    /// Enable HTTP/1 support.
    ///
    /// Enabled by default.
    pub fn enable_http1(self) -> Self {
        self.http1(true)
    }

    /// Disable HTTP/1 support.
    ///
    /// Enabled by default.
    pub fn disable_http1(self) -> Self {
        self.http1(false)
    }

    /// Enable or disable HTTP/2 support based on the given bool.
    ///
    /// Enabled by default.
    pub fn http2(mut self, enable: bool) -> Self {
        self.enable_http2 = enable;
        self
    }

    /// Enable HTTP/2 support.
    ///
    /// Enabled by default.
    pub fn enable_http2(self) -> Self {
        self.http2(true)
    }

    /// Disable HTTP/2 support.
    ///
    /// Enabled by default.
    pub fn disable_http2(self) -> Self {
        self.http2(false)
    }

    /// Enable or disable HTTP/3 support based on the given bool.
    ///
    /// Disabled by default.
    pub fn http3(mut self, enable: bool) -> Self {
        self.enable_http3 = enable;
        self
    }

    /// Enable HTTP/3 support.
    ///
    /// Disabled by default.
    pub fn enable_http3(self) -> Self {
        self.http3(true)
    }

    /// Disable HTTP/3 support.
    ///
    /// Disabled by default.
    pub fn disable_http3(self) -> Self {
        self.http3(false)
    }

    /// Set the rustls configuration for the server.
    ///
    /// This enables TLS support for the server.
    ///
    /// Required for HTTP/3 support.
    pub fn with_rustls(mut self, config: rustls::ServerConfig) -> Self {
        self.rustls_config = Some(config);
        self
    }

    /// Build the [`HttpServer`](crate::HttpServer) from this builder.
    ///
    /// Make sure to set the bind address and service factory before calling this method.
    /// If HTTP/3 support is enabled, the rustls configuration must be set as well.
    pub fn build(mut self) -> Result<HttpServer<F>, ServerBuilderError> {
        // https://www.iana.org/assignments/tls-extensiontype-values/tls-extensiontype-values.xhtml#alpn-protocol-ids
        if let Some(rustlsconfig) = &mut self.rustls_config {
            rustlsconfig.alpn_protocols.clear();

            if self.enable_http1 {
                rustlsconfig.alpn_protocols.push(b"http/1.0".to_vec());
                rustlsconfig.alpn_protocols.push(b"http/1.1".to_vec());
            }

            if self.enable_http2 {
                rustlsconfig.alpn_protocols.push(b"h2".to_vec());
                rustlsconfig.alpn_protocols.push(b"h2c".to_vec());
            }

            if self.enable_http3 {
                rustlsconfig.alpn_protocols.push(b"h3".to_vec());
            }
        } else if self.enable_http3 {
            // HTTP/3 doesn't work without TLS
            return Err(ServerBuilderError::MissingRustlsConfig);
        }

        Ok(HttpServer {
            ctx: self.ctx.unwrap_or_else(scuffle_context::Context::global),
            service_factory: self.service_factory.ok_or(ServerBuilderError::MissingServiceFactory)?,
            bind: self.bind.ok_or(ServerBuilderError::MissingBind)?,
            rustls_config: self.rustls_config,
            enable_http1: self.enable_http1,
            enable_http2: self.enable_http2,
            enable_http3: self.enable_http3,
        })
    }
}

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use std::convert::Infallible;
    use std::fs;
    use std::io::BufReader;

    use super::ServerBuilder;
    use crate::server::builder::ServerBuilderError;
    use crate::service::{fn_http_service, service_clone_factory};

    #[test]
    fn builder_missing_bind() {
        let builder = ServerBuilder::default().with_service_factory(service_clone_factory(fn_http_service(|_| async {
            Ok::<_, Infallible>(http::Response::new(RESPONSE_TEXT.to_string()))
        })));

        assert_eq!(builder.build().unwrap_err(), ServerBuilderError::MissingBind);
    }

    #[tokio::test]
    async fn builder_rustls() {
        rustls::crypto::aws_lc_rs::default_provider()
            .install_default()
            .expect("failed to install aws lc provider");

        let certfile = fs::File::open("assets/cert.pem").expect("cert not found");
        let certs = rustls_pemfile::certs(&mut BufReader::new(certfile))
            .collect::<Result<Vec<_>, _>>()
            .expect("failed to load certs");
        let keyfile = fs::File::open("assets/key.pem").expect("key not found");
        let key = rustls_pemfile::private_key(&mut BufReader::new(keyfile))
            .expect("failed to load key")
            .expect("no key found");

        let rustls_config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .expect("failed to build config");

        let (ctx, handler) = scuffle_context::Context::new();
        let addr = get_available_addr().expect("failed to get available address");

        let builder = ServerBuilder::default()
            .with_ctx(ctx)
            .with_service_factory(service_clone_factory(fn_http_service(|_| async {
                Ok::<_, Infallible>(http::Response::new(RESPONSE_TEXT.to_string()))
            })))
            .with_rustls(rustls_config)
            .bind(addr)
            .enable_http3();

        let server = builder.build().expect("failed to build server");

        let handle = tokio::spawn(async move {
            server.run().await.expect("server run failed");
        });

        // Wait for the server to start
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .expect("failed to build client");

        let resp = client
            .get(format!("https://{}/", addr))
            .send()
            .await
            .expect("failed to get response")
            .text()
            .await
            .expect("failed to get text");

        assert_eq!(resp, RESPONSE_TEXT);

        handler.shutdown().await;
        handle.await.expect("task failed");
    }

    #[test]
    fn builder_missing_rustls() {
        let builder = ServerBuilder::default()
            .with_service_factory(service_clone_factory(fn_http_service(|_| async {
                Ok::<_, Infallible>(http::Response::new(RESPONSE_TEXT.to_string()))
            })))
            .enable_http3();

        assert_eq!(builder.build().unwrap_err(), ServerBuilderError::MissingRustlsConfig);
    }

    fn get_available_addr() -> std::io::Result<std::net::SocketAddr> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        listener.local_addr()
    }

    const RESPONSE_TEXT: &str = "Hello, world!";

    #[tokio::test]
    async fn simple_server() {
        let mut builder = ServerBuilder::default().with_service_factory(service_clone_factory(fn_http_service(|_| async {
            Ok::<_, Infallible>(http::Response::new(RESPONSE_TEXT.to_string()))
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

        let (ctx, handler) = scuffle_context::Context::new();

        builder = builder.with_ctx(ctx);
        assert!(builder.ctx.is_some());

        let server = builder.build().unwrap();
        assert!(server.rustls_config.is_none());
        assert!(server.enable_http1);
        assert!(server.enable_http2);
        assert!(!server.enable_http3);

        let handle = tokio::spawn(async move {
            server.run().await.expect("server run failed");
        });

        // Wait for the server to start
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let resp = reqwest::get(format!("http://{}/", addr))
            .await
            .expect("failed to get response")
            .text()
            .await
            .expect("failed to get text");

        assert_eq!(resp, RESPONSE_TEXT);

        handler.shutdown().await;
        handle.await.expect("task failed");
    }
}
