use std::fmt::Debug;
use std::net::SocketAddr;

use super::HttpServer;
use crate::service::{
    custom_tower_make_service_factory, tower_make_service_factory, tower_make_service_with_addr_factory, HttpService,
    HttpServiceFactory, TowerMakeServiceFactory, TowerMakeServiceWithAddrFactory,
};
use crate::IncomingRequest;

#[derive(Debug, thiserror::Error)]
#[allow(clippy::enum_variant_names)]
pub enum ServerBuilderError {
    #[error("missing bind address")]
    MissingBind,
    #[error("missing service")]
    MissingServiceFactory,
    #[error("missing rustls configuration")]
    MissingRustlsConfig,
}

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
    pub fn with_custom_tower_make_service(mut self, tower_make_service: M, target: T) -> Self {
        self.service_factory = Some(custom_tower_make_service_factory(tower_make_service, target));
        self
    }
}

impl<F> ServerBuilder<F>
where
    F: HttpServiceFactory,
{
    pub fn with_ctx(mut self, ctx: scuffle_context::Context) -> Self {
        self.ctx = Some(ctx);
        self
    }

    pub fn bind(mut self, bind: SocketAddr) -> Self {
        self.bind = Some(bind);
        self
    }

    pub fn with_service_factory(mut self, service_factory: F) -> Self {
        self.service_factory = Some(service_factory);
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
        self.http1(false)
    }

    pub fn http2(mut self, enable: bool) -> Self {
        self.enable_http2 = enable;
        self
    }

    pub fn enable_http2(self) -> Self {
        self.http2(true)
    }

    pub fn disable_http2(self) -> Self {
        self.http2(false)
    }

    pub fn http3(mut self, enable: bool) -> Self {
        self.enable_http3 = enable;
        self
    }

    pub fn enable_http3(self) -> Self {
        self.http3(true)
    }

    pub fn disable_http3(self) -> Self {
        self.http3(false)
    }

    pub fn with_rustls(mut self, config: rustls::ServerConfig) -> Self {
        self.rustls_config = Some(config);
        self
    }

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
