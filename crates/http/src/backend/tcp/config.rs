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

#[cfg(feature = "http1")]
#[derive(Debug, Clone, Copy)]
#[must_use = "Http1Builder must be used to create a Http1Server"]
pub struct Http1Builder {
	/// The buffer size to use for underlying TCP stream. (default: 32KiB)
	pub recv_buffer_size: usize,
	/// The buffer size to use for sending data. (default: 32KiB)
	pub send_buffer_size: usize,
	/// The maximum header length. (default: 100)
	pub max_headers: usize,
	/// Whether to use half-close for the HTTP/1.1 connection. (default: false)
	pub half_close: bool,
	/// Whether to allow reusing the connection after a request. (default: true)
	pub keep_alive: bool,
	/// Whether to allow HTTP/1.0 requests. (default: false)
	pub allow_http10: bool,
}

#[cfg(feature = "http1")]
impl Default for Http1Builder {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(feature = "http1")]
impl Http1Builder {
	pub fn new() -> Self {
		Self {
			recv_buffer_size: 32 * 1024,
			send_buffer_size: 32 * 1024,
			max_headers: 100,
			half_close: false,
			keep_alive: true,
			allow_http10: false,
		}
	}

	pub fn with_recv_buffer_size(mut self, size: usize) -> Self {
		self.recv_buffer_size = size;
		self
	}

	pub fn with_send_buffer_size(mut self, size: usize) -> Self {
		self.send_buffer_size = size;
		self
	}

	pub fn with_max_header(mut self, size: usize) -> Self {
		self.max_headers = size;
		self
	}

	pub fn with_half_close(mut self, half_close: bool) -> Self {
		self.half_close = half_close;
		self
	}

	pub fn with_keep_alive(mut self, keep_alive: bool) -> Self {
		self.keep_alive = keep_alive;
		self
	}

	pub fn with_allow_http10(mut self, allow_http10: bool) -> Self {
		self.allow_http10 = allow_http10;
		self
	}
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
	#[cfg(feature = "http2")]
	pub http2_builder: Option<h2::server::Builder>,
	#[cfg(feature = "http1")]
	pub http1_builder: Option<Http1Builder>,
	pub acceptor: Option<TlsAcceptor>,
	/// The maximum time a connection can be idle before it is closed. (default:
	/// 30 seconds)
	pub idle_timeout: Option<std::time::Duration>,
	/// The maximum time a TLS handshake can take. (default: 5 seconds)
	pub handshake_timeout: Option<std::time::Duration>,
	pub server_name: Option<Arc<str>>,
	pub make_listener: MakeListener<std::net::TcpListener>,
}

impl TcpServerConfig {
	pub(crate) fn inner(&self) -> TcpServerConfigInner {
		TcpServerConfigInner {
			idle_timeout: self.idle_timeout,
			handshake_timeout: self.handshake_timeout,
			server_name: self.server_name.clone(),
			#[cfg(feature = "http1")]
			http1_builder: self.http1_builder,
			#[cfg(feature = "http2")]
			http2_builder: self.http2_builder.clone(),
		}
	}
}

#[derive(Debug, Clone)]
pub(crate) struct TcpServerConfigInner {
	pub idle_timeout: Option<std::time::Duration>,
	#[cfg_attr(not(feature = "tls-rustls"), allow(unused))]
	pub handshake_timeout: Option<std::time::Duration>,
	pub server_name: Option<Arc<str>>,
	#[cfg(feature = "http1")]
	pub http1_builder: Option<Http1Builder>,
	#[cfg(feature = "http2")]
	pub http2_builder: Option<h2::server::Builder>,
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

#[must_use = "TcpServerConfigBuilder must be built to create a TcpServerConfig"]
pub struct TcpServerConfigBuilder<A = (), L = ()> {
	#[cfg(feature = "http2")]
	http2_builder: Option<h2::server::Builder>,
	#[cfg(feature = "http1")]
	http1_builder: Option<Http1Builder>,
	listener: L,
	acceptor: A,
	connection_limit: Option<usize>,
	idle_timeout: Option<std::time::Duration>,
	handshake_timeout: Option<std::time::Duration>,
	server_name: Option<Arc<str>>,
}

impl Default for TcpServerConfigBuilder {
	fn default() -> Self {
		Self::new()
	}
}

impl TcpServerConfigBuilder {
	pub fn new() -> Self {
		Self {
			#[cfg(feature = "http2")]
			http2_builder: Some(h2::server::Builder::new()),
			#[cfg(feature = "http1")]
			http1_builder: Some(Http1Builder::new()),
			listener: (),
			acceptor: (),
			connection_limit: None,
			idle_timeout: Some(std::time::Duration::from_secs(30)),
			handshake_timeout: Some(std::time::Duration::from_secs(5)),
			server_name: None,
		}
	}
}

impl<A> TcpServerConfigBuilder<A, ()> {
	pub fn with_bind(self, addr: SocketAddr) -> TcpServerConfigBuilder<A, MakeListener<std::net::TcpListener>> {
		TcpServerConfigBuilder {
			#[cfg(feature = "http2")]
			http2_builder: self.http2_builder,
			#[cfg(feature = "http1")]
			http1_builder: self.http1_builder,
			listener: MakeListener::bind(addr),
			acceptor: self.acceptor,
			connection_limit: self.connection_limit,
			idle_timeout: self.idle_timeout,
			handshake_timeout: self.handshake_timeout,
			server_name: self.server_name,
		}
	}

	pub fn with_listener(
		self,
		listener: std::net::TcpListener,
	) -> TcpServerConfigBuilder<A, MakeListener<std::net::TcpListener>> {
		TcpServerConfigBuilder {
			#[cfg(feature = "http2")]
			http2_builder: self.http2_builder,
			#[cfg(feature = "http1")]
			http1_builder: self.http1_builder,
			listener: MakeListener::listener(listener),
			acceptor: self.acceptor,
			connection_limit: self.connection_limit,
			idle_timeout: self.idle_timeout,
			handshake_timeout: self.handshake_timeout,
			server_name: self.server_name,
		}
	}

	pub fn with_make_listener(
		self,
		make_listener: impl Fn() -> std::io::Result<std::net::TcpListener> + 'static + Send,
	) -> TcpServerConfigBuilder<A, MakeListener<std::net::TcpListener>> {
		TcpServerConfigBuilder {
			#[cfg(feature = "http2")]
			http2_builder: self.http2_builder,
			#[cfg(feature = "http1")]
			http1_builder: self.http1_builder,
			listener: MakeListener::custom(make_listener),
			acceptor: self.acceptor,
			connection_limit: self.connection_limit,
			idle_timeout: self.idle_timeout,
			handshake_timeout: self.handshake_timeout,
			server_name: self.server_name,
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
			#[cfg(feature = "http2")]
			http2_builder: self.http2_builder,
			#[cfg(feature = "http1")]
			http1_builder: self.http1_builder,
			listener: self.listener,
			acceptor: acceptor.into(),
			connection_limit: self.connection_limit,
			idle_timeout: self.idle_timeout,
			handshake_timeout: self.handshake_timeout,
			server_name: self.server_name,
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
	pub fn with_http2_builder(self, builder: h2::server::Builder) -> Self {
		Self {
			#[cfg(feature = "http2")]
			http2_builder: Some(builder),
			#[cfg(feature = "http1")]
			http1_builder: self.http1_builder,
			listener: self.listener,
			acceptor: self.acceptor,
			connection_limit: self.connection_limit,
			idle_timeout: self.idle_timeout,
			handshake_timeout: self.handshake_timeout,
			server_name: self.server_name,
		}
	}

	pub fn with_http2_builder_fn(self, builder: impl FnOnce() -> h2::server::Builder) -> Self {
		Self {
			#[cfg(feature = "http2")]
			http2_builder: Some(builder()),
			#[cfg(feature = "http1")]
			http1_builder: self.http1_builder,
			listener: self.listener,
			acceptor: self.acceptor,
			connection_limit: self.connection_limit,
			idle_timeout: self.idle_timeout,
			handshake_timeout: self.handshake_timeout,
			server_name: self.server_name,
		}
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

	pub fn with_http1_builder(mut self, builder: Http1Builder) -> Self {
		self.http1_builder = Some(builder);
		self
	}

	pub fn disable_http2(mut self) -> Self {
		self.http2_builder = None;
		self
	}

	pub fn disable_http1(mut self) -> Self {
		self.http1_builder = None;
		self
	}

	pub fn with_http1_builder_fn(mut self, builder: impl FnOnce() -> Http1Builder) -> Self {
		self.http1_builder = Some(builder());
		self
	}

	pub fn with_server_name(mut self, server_name: impl Into<Arc<str>>) -> Self {
		self.server_name = Some(server_name.into());
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
			#[cfg(feature = "http2")]
			http2_builder: self.http2_builder,
			#[cfg(feature = "http1")]
			http1_builder: self.http1_builder,
			make_listener: self.listener,
			idle_timeout: self.idle_timeout,
			handshake_timeout: self.handshake_timeout,
			server_name: self.server_name,
			acceptor: self.acceptor.into_tls_acceptor(),
		}
	}
}
