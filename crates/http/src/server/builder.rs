use std::{fmt::{Debug, Display}, net::SocketAddr};

use super::HttpServer;

#[derive(Debug, thiserror::Error)]
pub enum ServerBuilderError {
    #[error("missing bind address")]
    MissingBind,
    #[error("missing service")]
    MissingService,
    #[error("missing rustls configuration")]
    MissingRustlsConfig,
}

pub struct ServerBuilder<M> {
    ctx: Option<scuffle_context::Context>,
    bind: Option<SocketAddr>,
    service: Option<M>,
    rustls_config: Option<rustls::ServerConfig>,
    enable_http1: bool,
    enable_http2: bool,
    enable_http3: bool,
}

impl<M> Default for ServerBuilder<M> {
    fn default() -> Self {
        Self {
            ctx: None,
            bind: None,
            service: None,
            rustls_config: None,
            enable_http1: true,
            enable_http2: true,
            enable_http3: false,
        }
    }
}

impl<M, B> ServerBuilder<M>
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
    pub fn with_ctx(mut self, ctx: scuffle_context::Context) -> Self {
        self.ctx = Some(ctx);
        self
    }

    pub fn bind(mut self, bind: SocketAddr) -> Self {
        self.bind = Some(bind);
        self
    }

    pub fn with_service(mut self, service: M) -> Self {
        self.service = Some(service);
        self
    }

    pub fn http1(mut self, enable: bool) -> Self {
        self.enable_http1 = enable;
        self
    }

    pub fn enable_http1(self) -> Self {
        self.http1(true)
    }

    pub fn disable_http1(self) -> Self {
        self.http1(true)
    }

    pub fn http2(mut self, enable: bool) -> Self {
        self.enable_http2 = enable;
        self
    }

    pub fn enable_http2(self) -> Self {
        self.http2(true)
    }

    pub fn disable_http2(self) -> Self {
        self.http2(true)
    }

    pub fn http3(mut self, enable: bool) -> Self {
        self.enable_http3 = enable;
        self
    }

    pub fn enable_http3(self) -> Self {
        self.http3(true)
    }

    pub fn disable_http3(self) -> Self {
        self.http3(true)
    }

    pub fn with_rustls(mut self, config: rustls::ServerConfig) -> Self {
        self.rustls_config = Some(config);
        self
    }

    pub fn build(mut self) -> Result<HttpServer<M>, ServerBuilderError> {
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
            service: self.service.ok_or(ServerBuilderError::MissingService)?,
            bind: self.bind.ok_or(ServerBuilderError::MissingBind)?,
            rustls_config: self.rustls_config,
            enable_http1: self.enable_http1,
            enable_http2: self.enable_http2,
            enable_http3: false,
        })
    }
}
