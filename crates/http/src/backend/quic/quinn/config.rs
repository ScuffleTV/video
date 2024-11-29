use std::net::SocketAddr;
use std::sync::Arc;

use crate::builder::MakeListener;

#[derive(Debug, Clone)]
pub enum QuinnAcceptorVerdict {
	/// Accept the connection
	Accept(Option<Arc<quinn::ServerConfig>>),
	/// Implicitly reject the connection
	Refuse,
	/// Ignore the connection, not sending any packet in response
	Ignore,
}

#[async_trait::async_trait]
pub trait LazyQuinnAcceptor: Send + Sync + 'static {
	async fn accept(&self) -> QuinnAcceptorVerdict;
}

#[derive(derive_more::Debug)]
#[must_use = "QuinnServerConfig must be used to create a QuinnServer"]
pub struct QuinnServerConfig {
	/// The maximum time a connection can be idle before it is closed. (default:
	/// 30 seconds)
	pub idle_timeout: Option<std::time::Duration>,
	/// The maximum time a TLS handshake can take. (default: 5 seconds)
	pub handshake_timeout: Option<std::time::Duration>,
	#[debug(skip)]
	pub http_builder: Arc<h3::server::Builder>,
	#[debug(skip)]
	pub quinn_config: quinn::ServerConfig,
	#[debug(skip)]
	pub quinn_dynamic_config: Option<Arc<dyn LazyQuinnAcceptor>>,
	#[debug(skip)]
	pub endpoint_config: quinn::EndpointConfig,
	pub make_listener: MakeListener<std::net::UdpSocket>,
}

impl QuinnServerConfig {
	pub(crate) fn inner(&self) -> QuinnServerConfigInner {
		QuinnServerConfigInner {
			idle_timeout: self.idle_timeout,
			handshake_timeout: self.handshake_timeout,
			quinn_dynamic_config: self.quinn_dynamic_config.clone(),
			http_builder: self.http_builder.clone(),
		}
	}
}

pub(crate) struct QuinnServerConfigInner {
	pub idle_timeout: Option<std::time::Duration>,
	pub handshake_timeout: Option<std::time::Duration>,
	pub quinn_dynamic_config: Option<Arc<dyn LazyQuinnAcceptor>>,
	pub http_builder: Arc<h3::server::Builder>,
}

impl QuinnServerConfig {
	pub fn builder() -> QuinnServerConfigBuilder {
		QuinnServerConfigBuilder::new()
	}

	pub fn into_server(self) -> super::QuinnServer {
		super::QuinnServer::new(self)
	}
}

pub fn builder() -> QuinnServerConfigBuilder {
	QuinnServerConfigBuilder::new()
}

#[must_use = "QuinnServerConfigBuilder must be built to create a QuinnServerConfig"]
pub struct QuinnServerConfigBuilder<C = (), L = ()> {
	http_builder: h3::server::Builder,
	endpoint_config: quinn::EndpointConfig,
	idle_timeout: Option<std::time::Duration>,
	handshake_timeout: Option<std::time::Duration>,
	quinn_config: C,
	quinn_dynamic_config: Option<Arc<dyn LazyQuinnAcceptor>>,
	listener: L,
}

impl Default for QuinnServerConfigBuilder {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(feature = "tls-rustls")]
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub enum QuinnBackendBuilderError {
	NoInitialCipherSuite(#[from] quinn::crypto::rustls::NoInitialCipherSuite),
	#[cfg(feature = "tls-rustls-pem")]
	Io(#[from] std::io::Error),
	#[cfg(feature = "tls-rustls-pem")]
	Rustls(#[from] rustls::Error),
}

impl QuinnServerConfigBuilder {
	pub fn new() -> Self {
		Self {
			http_builder: h3::server::builder(),
			endpoint_config: quinn::EndpointConfig::default(),
			quinn_config: (),
			quinn_dynamic_config: None,
			listener: (),
			idle_timeout: Some(std::time::Duration::from_secs(30)),
			handshake_timeout: Some(std::time::Duration::from_secs(5)),
		}
	}
}

impl<L> QuinnServerConfigBuilder<(), L> {
	#[cfg(feature = "tls-rustls")]
	pub fn with_rustls_config(
		self,
		config: Arc<rustls::ServerConfig>,
	) -> Result<QuinnServerConfigBuilder<quinn::ServerConfig, L>, QuinnBackendBuilderError> {
		use quinn::crypto::rustls::QuicServerConfig;

		Ok(
			self.with_quinn_config(quinn::ServerConfig::with_crypto(Arc::new(QuicServerConfig::try_from(
				config,
			)?))),
		)
	}

	#[cfg(feature = "tls-rustls-pem")]
	pub fn with_tls_from_pem(
		self,
		cert: impl AsRef<[u8]>,
		key: impl AsRef<[u8]>,
	) -> Result<QuinnServerConfigBuilder<quinn::ServerConfig, L>, QuinnBackendBuilderError> {
		let cert_chain = rustls_pemfile::certs(&mut std::io::Cursor::new(cert.as_ref())).collect::<Result<Vec<_>, _>>()?;
		let key = rustls_pemfile::private_key(&mut std::io::Cursor::new(key.as_ref()))?
			.ok_or(std::io::Error::new(std::io::ErrorKind::InvalidInput, "no key found"))?;

		let mut tls = rustls::ServerConfig::builder_with_provider(rustls::crypto::ring::default_provider().into())
			.with_protocol_versions(&[&rustls::version::TLS13])?
			.with_no_client_auth()
			.with_single_cert(cert_chain, key)?;

		tls.alpn_protocols = vec![
			b"h3".to_vec(),
			b"hq".to_vec(),
			b"h3-29".to_vec(),
			b"hq-29".to_vec(),
			b"h3-32".to_vec(),
			b"h3-31".to_vec(),
			b"h3-30".to_vec(),
			b"h3-29".to_vec(),
		];
		tls.max_early_data_size = u32::MAX;

		self.with_rustls_config(Arc::new(tls))
	}

	pub fn with_quinn_config(self, config: quinn::ServerConfig) -> QuinnServerConfigBuilder<quinn::ServerConfig, L> {
		QuinnServerConfigBuilder {
			http_builder: self.http_builder,
			quinn_config: config,
			quinn_dynamic_config: None,
			listener: self.listener,
			endpoint_config: self.endpoint_config,
			idle_timeout: self.idle_timeout,
			handshake_timeout: self.handshake_timeout,
		}
	}
}

impl<L> QuinnServerConfigBuilder<quinn::ServerConfig, L> {
	pub fn modify_quinn_config(mut self, modify: impl FnOnce(&mut quinn::ServerConfig)) -> Self {
		modify(&mut self.quinn_config);
		self
	}
}

impl<C> QuinnServerConfigBuilder<C, ()> {
	pub fn with_bind(self, addr: SocketAddr) -> QuinnServerConfigBuilder<C, MakeListener<std::net::UdpSocket>> {
		QuinnServerConfigBuilder {
			http_builder: self.http_builder,
			quinn_config: self.quinn_config,
			quinn_dynamic_config: self.quinn_dynamic_config,
			endpoint_config: self.endpoint_config,
			listener: MakeListener::bind(addr),
			idle_timeout: self.idle_timeout,
			handshake_timeout: self.handshake_timeout,
		}
	}

	pub fn with_listener(
		self,
		listener: std::net::UdpSocket,
	) -> QuinnServerConfigBuilder<C, MakeListener<std::net::UdpSocket>> {
		QuinnServerConfigBuilder {
			http_builder: self.http_builder,
			quinn_config: self.quinn_config,
			quinn_dynamic_config: self.quinn_dynamic_config,
			endpoint_config: self.endpoint_config,
			listener: MakeListener::listener(listener),
			idle_timeout: self.idle_timeout,
			handshake_timeout: self.handshake_timeout,
		}
	}

	pub fn with_make_listener(
		self,
		make_listener: impl Fn() -> std::io::Result<std::net::UdpSocket> + Send + 'static,
	) -> QuinnServerConfigBuilder<C, MakeListener<std::net::UdpSocket>> {
		QuinnServerConfigBuilder {
			http_builder: self.http_builder,
			quinn_config: self.quinn_config,
			quinn_dynamic_config: self.quinn_dynamic_config,
			endpoint_config: self.endpoint_config,
			listener: MakeListener::custom(make_listener),
			idle_timeout: self.idle_timeout,
			handshake_timeout: self.handshake_timeout,
		}
	}
}

impl<C, L> QuinnServerConfigBuilder<C, L> {
	pub fn with_http_builder(self, builder: h3::server::Builder) -> QuinnServerConfigBuilder<C, L> {
		QuinnServerConfigBuilder {
			http_builder: builder,
			quinn_config: self.quinn_config,
			quinn_dynamic_config: self.quinn_dynamic_config,
			listener: self.listener,
			endpoint_config: self.endpoint_config,
			idle_timeout: self.idle_timeout,
			handshake_timeout: self.handshake_timeout,
		}
	}

	pub fn with_http_builder_fn(self, builder: impl Fn() -> h3::server::Builder) -> QuinnServerConfigBuilder<C, L> {
		QuinnServerConfigBuilder {
			http_builder: builder(),
			quinn_config: self.quinn_config,
			quinn_dynamic_config: self.quinn_dynamic_config,
			listener: self.listener,
			endpoint_config: self.endpoint_config,
			idle_timeout: self.idle_timeout,
			handshake_timeout: self.handshake_timeout,
		}
	}

	pub fn with_quinn_acceptor(self, acceptor: impl LazyQuinnAcceptor) -> QuinnServerConfigBuilder<C, L> {
		QuinnServerConfigBuilder {
			http_builder: self.http_builder,
			quinn_config: self.quinn_config,
			quinn_dynamic_config: Some(Arc::new(acceptor)),
			listener: self.listener,
			endpoint_config: self.endpoint_config,
			idle_timeout: self.idle_timeout,
			handshake_timeout: self.handshake_timeout,
		}
	}

	pub fn modify_endpoint_config(mut self, modify: impl FnOnce(&mut quinn::EndpointConfig)) -> Self {
		modify(&mut self.endpoint_config);
		self
	}

	pub fn modify_http_builder(mut self, modify: impl FnOnce(&mut h3::server::Builder)) -> Self {
		modify(&mut self.http_builder);
		self
	}

	pub fn with_idle_timeout(mut self, idle_timeout: impl Into<Option<std::time::Duration>>) -> Self {
		self.idle_timeout = idle_timeout.into();
		self
	}

	pub fn with_handshake_timeout(mut self, handshake_timeout: impl Into<Option<std::time::Duration>>) -> Self {
		self.handshake_timeout = handshake_timeout.into();
		self
	}
}

impl QuinnServerConfigBuilder<quinn::ServerConfig, MakeListener<std::net::UdpSocket>> {
	pub fn build(self) -> QuinnServerConfig {
		QuinnServerConfig {
			http_builder: Arc::new(self.http_builder),
			quinn_config: self.quinn_config,
			quinn_dynamic_config: self.quinn_dynamic_config,
			make_listener: self.listener,
			endpoint_config: self.endpoint_config,
			idle_timeout: self.idle_timeout,
			handshake_timeout: self.handshake_timeout,
		}
	}
}
