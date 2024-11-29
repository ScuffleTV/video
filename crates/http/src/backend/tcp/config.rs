use std::net::SocketAddr;
use std::sync::Arc;

use super::TcpServer;
use crate::builder::MakeListener;

#[derive(derive_more::Debug, Clone)]
pub enum TlsAcceptor {
	#[cfg(feature = "tls-rustls")]
	#[debug("Rustls")]
	Rustls(Arc<rustls::ServerConfig>),
	#[cfg(feature = "tls-rustls")]
	#[debug("RustlsLazy")]
	RustlsLazy(Arc<dyn RustlsLazyAcceptor>),
}

#[cfg(feature = "tls-rustls")]
impl TlsAcceptor {
	pub(crate) async fn accept(&self, client_hello: rustls::server::ClientHello<'_>) -> Option<Arc<rustls::ServerConfig>> {
		match self {
			TlsAcceptor::Rustls(acceptor) => Some(acceptor.clone()),
			TlsAcceptor::RustlsLazy(acceptor) => acceptor.accept(client_hello).await,
		}
	}

	pub fn set_alpn(&mut self, alpn: Vec<Vec<u8>>) {
		match self {
			TlsAcceptor::Rustls(acceptor) => {
				if acceptor.alpn_protocols.is_empty() {
					if let Some(config) = Arc::get_mut(acceptor) {
						config.alpn_protocols = alpn;
					} else {
						let mut config = (**acceptor).clone();
						config.alpn_protocols = alpn;
						*acceptor = Arc::new(config);
					}
				}
			}
			TlsAcceptor::RustlsLazy(_) => (),
		}
	}
}

#[cfg(feature = "tls-rustls")]
#[async_trait::async_trait]
pub trait RustlsLazyAcceptor: Send + Sync {
	async fn accept(&self, client_hello: rustls::server::ClientHello<'_>) -> Option<Arc<rustls::ServerConfig>>;
}

#[cfg(feature = "tls-rustls")]
impl From<rustls::ServerConfig> for TlsAcceptor {
	fn from(config: rustls::ServerConfig) -> Self {
		TlsAcceptor::Rustls(Arc::new(config))
	}
}

#[cfg(feature = "tls-rustls")]
impl From<Arc<rustls::ServerConfig>> for TlsAcceptor {
	fn from(config: Arc<rustls::ServerConfig>) -> Self {
		TlsAcceptor::Rustls(config)
	}
}

#[cfg(feature = "tls-rustls")]
impl From<Arc<dyn RustlsLazyAcceptor>> for TlsAcceptor {
	fn from(acceptor: Arc<dyn RustlsLazyAcceptor>) -> Self {
		TlsAcceptor::RustlsLazy(acceptor)
	}
}

#[cfg(feature = "tls-rustls")]
impl<A: RustlsLazyAcceptor + 'static> From<A> for TlsAcceptor {
	fn from(acceptor: A) -> Self {
		TlsAcceptor::RustlsLazy(Arc::new(acceptor))
	}
}

#[derive(Debug)]
#[must_use = "TcpServerConfig must be used to create a TcpServer"]
pub struct TcpServerConfig {
	pub http_builder: hyper_util::server::conn::auto::Builder<hyper_util::rt::TokioExecutor>,
	pub acceptor: Option<TlsAcceptor>,
	/// The maximum time a connection can be idle before it is closed. (default:
	/// 30 seconds)
	pub idle_timeout: Option<std::time::Duration>,
	/// The maximum time a TLS handshake can take. (default: 5 seconds)
	pub handshake_timeout: Option<std::time::Duration>,
	pub server_name: Option<Arc<str>>,
	pub allow_upgrades: bool,
	pub only_http: Option<HttpVersion>,
	pub make_listener: MakeListener<std::net::TcpListener>,
}

impl TcpServerConfig {
	pub(crate) fn inner(&self) -> TcpServerConfigInner {
		TcpServerConfigInner {
			idle_timeout: self.idle_timeout,
			handshake_timeout: self.handshake_timeout,
			server_name: self.server_name.clone(),
			allow_upgrades: self.allow_upgrades,
			http_builder: self.http_builder.clone(),
		}
	}
}

#[derive(Debug, Clone)]
pub(crate) struct TcpServerConfigInner {
	pub idle_timeout: Option<std::time::Duration>,
	#[cfg_attr(not(feature = "tls-rustls"), allow(unused))]
	pub handshake_timeout: Option<std::time::Duration>,
	pub server_name: Option<Arc<str>>,
	pub allow_upgrades: bool,
	pub http_builder: hyper_util::server::conn::auto::Builder<hyper_util::rt::TokioExecutor>,
}

pub fn builder() -> TcpServerConfigBuilder {
	TcpServerConfigBuilder::new()
}

impl TcpServerConfig {
	pub fn builder() -> TcpServerConfigBuilder {
		TcpServerConfigBuilder::new()
	}

	pub fn into_server(self) -> TcpServer {
		TcpServer::new(self)
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpVersion {
	#[cfg(feature = "http2")]
	Http2,
	#[cfg(feature = "http1")]
	Http1,
}

#[must_use = "TcpServerConfigBuilder must be built to create a TcpServerConfig"]
pub struct TcpServerConfigBuilder<A = (), L = ()> {
	http_builder: hyper_util::server::conn::auto::Builder<hyper_util::rt::TokioExecutor>,
	listener: L,
	acceptor: A,
	connection_limit: Option<usize>,
	idle_timeout: Option<std::time::Duration>,
	handshake_timeout: Option<std::time::Duration>,
	server_name: Option<Arc<str>>,
	allow_upgrades: bool,
	only_http: Option<HttpVersion>,
}

impl Default for TcpServerConfigBuilder {
	fn default() -> Self {
		Self::new()
	}
}

impl TcpServerConfigBuilder {
	pub fn new() -> Self {
		Self {
			http_builder: hyper_util::server::conn::auto::Builder::new(hyper_util::rt::TokioExecutor::new()),
			listener: (),
			acceptor: (),
			connection_limit: None,
			idle_timeout: Some(std::time::Duration::from_secs(30)),
			handshake_timeout: Some(std::time::Duration::from_secs(5)),
			server_name: None,
			allow_upgrades: true,
			only_http: None,
		}
	}
}

impl<A> TcpServerConfigBuilder<A, ()> {
	pub fn with_bind(self, addr: SocketAddr) -> TcpServerConfigBuilder<A, MakeListener<std::net::TcpListener>> {
		TcpServerConfigBuilder {
			http_builder: self.http_builder,
			listener: MakeListener::bind(addr),
			acceptor: self.acceptor,
			connection_limit: self.connection_limit,
			idle_timeout: self.idle_timeout,
			handshake_timeout: self.handshake_timeout,
			server_name: self.server_name,
			allow_upgrades: self.allow_upgrades,
			only_http: self.only_http,
		}
	}

	pub fn with_listener(
		self,
		listener: std::net::TcpListener,
	) -> TcpServerConfigBuilder<A, MakeListener<std::net::TcpListener>> {
		TcpServerConfigBuilder {
			http_builder: self.http_builder,
			listener: MakeListener::listener(listener),
			acceptor: self.acceptor,
			connection_limit: self.connection_limit,
			idle_timeout: self.idle_timeout,
			handshake_timeout: self.handshake_timeout,
			server_name: self.server_name,
			allow_upgrades: self.allow_upgrades,
			only_http: self.only_http,
		}
	}

	pub fn with_make_listener(
		self,
		make_listener: impl Fn() -> std::io::Result<std::net::TcpListener> + 'static + Send,
	) -> TcpServerConfigBuilder<A, MakeListener<std::net::TcpListener>> {
		TcpServerConfigBuilder {
			http_builder: self.http_builder,
			listener: MakeListener::custom(make_listener),
			acceptor: self.acceptor,
			connection_limit: self.connection_limit,
			idle_timeout: self.idle_timeout,
			handshake_timeout: self.handshake_timeout,
			server_name: self.server_name,
			allow_upgrades: self.allow_upgrades,
			only_http: self.only_http,
		}
	}
}

#[cfg(feature = "tls-rustls-pem")]
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub enum HyperBackendBuilderError {
	#[cfg(feature = "tls-rustls-pem")]
	Io(#[from] std::io::Error),
	#[cfg(feature = "tls-rustls-pem")]
	Rustls(#[from] rustls::Error),
}

impl<L> TcpServerConfigBuilder<(), L> {
	#[cfg(feature = "tls-rustls")]
	pub fn with_tls_acceptor(self, acceptor: impl Into<TlsAcceptor>) -> TcpServerConfigBuilder<TlsAcceptor, L> {
		TcpServerConfigBuilder {
			acceptor: acceptor.into(),
			http_builder: self.http_builder,
			listener: self.listener,
			connection_limit: self.connection_limit,
			idle_timeout: self.idle_timeout,
			handshake_timeout: self.handshake_timeout,
			server_name: self.server_name,
			allow_upgrades: self.allow_upgrades,
			only_http: self.only_http,
		}
	}

	#[cfg(feature = "tls-rustls-pem")]
	pub fn with_tls_from_pem(
		self,
		cert: impl AsRef<[u8]>,
		key: impl AsRef<[u8]>,
	) -> Result<TcpServerConfigBuilder<TlsAcceptor, L>, HyperBackendBuilderError> {
		let cert_chain = rustls_pemfile::certs(&mut std::io::Cursor::new(cert.as_ref())).collect::<Result<Vec<_>, _>>()?;
		let key = rustls_pemfile::private_key(&mut std::io::Cursor::new(key.as_ref()))?
			.ok_or(std::io::Error::new(std::io::ErrorKind::InvalidInput, "no key found"))?;

		let config = rustls::ServerConfig::builder_with_provider(rustls::crypto::aws_lc_rs::default_provider().into())
			.with_safe_default_protocol_versions()?
			.with_no_client_auth()
			.with_single_cert(cert_chain, key)?;

		Ok(self.with_tls_acceptor(config))
	}
}

impl<C, L> TcpServerConfigBuilder<C, L> {
	pub fn with_http_builder(
		mut self,
		builder: hyper_util::server::conn::auto::Builder<hyper_util::rt::TokioExecutor>,
	) -> Self {
		self.http_builder = builder;
		self
	}

	pub fn with_http_builder_fn(
		mut self,
		builder: impl FnOnce() -> hyper_util::server::conn::auto::Builder<hyper_util::rt::TokioExecutor>,
	) -> Self {
		self.http_builder = builder();
		self
	}

	pub fn modify_http_builder(
		mut self,
		f: impl FnOnce(&mut hyper_util::server::conn::auto::Builder<hyper_util::rt::TokioExecutor>),
	) -> Self {
		f(&mut self.http_builder);
		self
	}

	#[cfg(feature = "http2")]
	pub fn http2_only(mut self) -> Self {
		self.only_http = Some(HttpVersion::Http2);
		self.http_builder = self.http_builder.http2_only();
		self
	}

	#[cfg(feature = "http1")]
	pub fn http1_only(mut self) -> Self {
		self.only_http = Some(HttpVersion::Http1);
		self.http_builder = self.http_builder.http1_only();
		self
	}

	pub fn with_connection_limit(mut self, limit: usize) -> Self {
		self.connection_limit = Some(limit);
		self
	}

	pub fn with_idle_timeout(mut self, timeout: std::time::Duration) -> Self {
		self.idle_timeout = Some(timeout);
		self
	}

	pub fn with_handshake_timeout(mut self, timeout: std::time::Duration) -> Self {
		self.handshake_timeout = Some(timeout);
		self
	}

	pub fn with_server_name(mut self, server_name: impl Into<Arc<str>>) -> Self {
		self.server_name = Some(server_name.into());
		self
	}

	pub fn with_allow_upgrades(mut self, allow_upgrades: bool) -> Self {
		self.allow_upgrades = allow_upgrades;
		self
	}
}
trait MaybeTlsAcceptor {
	fn into_tls_acceptor(self) -> Option<TlsAcceptor>;
}

impl MaybeTlsAcceptor for () {
	fn into_tls_acceptor(self) -> Option<TlsAcceptor> {
		None
	}
}

impl MaybeTlsAcceptor for TlsAcceptor {
	fn into_tls_acceptor(self) -> Option<TlsAcceptor> {
		Some(self)
	}
}

#[allow(private_bounds)]
impl<A: MaybeTlsAcceptor> TcpServerConfigBuilder<A, MakeListener<std::net::TcpListener>> {
	pub fn build(self) -> TcpServerConfig {
		TcpServerConfig {
			http_builder: self.http_builder,
			make_listener: self.listener,
			idle_timeout: self.idle_timeout,
			handshake_timeout: self.handshake_timeout,
			server_name: self.server_name,
			acceptor: self.acceptor.into_tls_acceptor(),
			allow_upgrades: self.allow_upgrades,
			only_http: self.only_http,
		}
	}
}
