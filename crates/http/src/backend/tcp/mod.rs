#[cfg(feature = "http1")]
mod http1;

#[cfg(feature = "http2")]
mod http2;

mod util;

pub(crate) enum TcpIncomingBody {
	#[cfg(feature = "http1")]
	Http1(http1::io::RecvStream),
	#[cfg(feature = "http2")]
	Http2(http2::io::RecvStream),
}

impl http_body::Body for TcpIncomingBody {
	type Data = Bytes;
	type Error = crate::Error;

	fn is_end_stream(&self) -> bool {
		match self {
			#[cfg(feature = "http1")]
			TcpIncomingBody::Http1(body) => body.is_end_stream(),
			#[cfg(feature = "http2")]
			TcpIncomingBody::Http2(body) => body.is_end_stream(),
		}
	}

	fn poll_frame(
		mut self: std::pin::Pin<&mut Self>,
		cx: &mut std::task::Context<'_>,
	) -> std::task::Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
		match &mut *self {
			#[cfg(feature = "http1")]
			TcpIncomingBody::Http1(body) => Pin::new(body).poll_frame(cx).map_err(Into::into),
			#[cfg(feature = "http2")]
			TcpIncomingBody::Http2(body) => Pin::new(body).poll_frame(cx).map_err(Into::into),
		}
	}

	fn size_hint(&self) -> http_body::SizeHint {
		match self {
			#[cfg(feature = "http1")]
			TcpIncomingBody::Http1(body) => body.size_hint(),
			#[cfg(feature = "http2")]
			TcpIncomingBody::Http2(body) => body.size_hint(),
		}
	}
}

pub mod config;

use std::pin::Pin;
use std::sync::Arc;

use bytes::Bytes;
pub use config::TcpServerConfig;
use scuffle_context::ContextFutExt;
use serve::serve_tcp;
use tokio::sync::Mutex;

use super::HttpServer;
use crate::svc::ConnectionAcceptor;
use crate::util::AbortOnDrop;

mod serve;

#[derive(Debug, thiserror::Error)]
pub enum TcpServerError {
	#[error(transparent)]
	JoinError(#[from] tokio::task::JoinError),
	#[error("io: {0}")]
	Io(#[from] std::io::Error),
	#[error("not started")]
	NotStarted,
}

#[derive(Clone)]
struct StartGroup {
	handler: scuffle_context::Handler,
	address: std::net::SocketAddr,
	#[allow(clippy::type_complexity)]
	threads: Arc<Mutex<Vec<AbortOnDrop<Option<Result<(), TcpServerError>>>>>>,
}

impl StartGroup {
	async fn wait(&self) -> Result<(), TcpServerError> {
		let mut threads = self.threads.lock().await;

		while !threads.is_empty() {
			let (result, _, remaining) = futures::future::select_all(threads.drain(..).map(|thread| thread.disarm())).await;
			*threads = remaining.into_iter().map(AbortOnDrop::new).collect();
			if let Some(Err(result)) = result? {
				return Err(result);
			}
		}

		Ok(())
	}
}

pub struct TcpServer {
	config: Mutex<TcpServerConfig>,
	start_group: spin::Mutex<Option<StartGroup>>,
}

impl TcpServer {
	pub fn new(config: TcpServerConfig) -> Self {
		Self {
			config: Mutex::new(config),
			start_group: spin::Mutex::new(None),
		}
	}
}

impl HttpServer for TcpServer {
	type Error = TcpServerError;

	async fn start<S: ConnectionAcceptor + Clone>(&self, service: S, workers: usize) -> Result<(), Self::Error> {
		let mut config = self.config.lock().await;
		let mut group = self.start_group.lock().take();
		if let Some(group) = group.take() {
			group.handler.cancel();
		}

		let listener = config.make_listener.make()?;
		listener.set_nonblocking(true)?;

		let address = listener.local_addr()?;

		let listeners = (0..workers).map(|_| listener.try_clone()).collect::<Result<Vec<_>, _>>()?;

		let handler = scuffle_context::Handler::new();

		#[cfg(all(feature = "http1", feature = "tls-rustls"))]
		let allow_http10 = config.http1_builder.as_ref().map_or(false, |b| b.allow_http10);
		#[cfg(all(feature = "http1", feature = "tls-rustls"))]
		let allow_http1 = config.http1_builder.is_some();
		#[cfg(all(feature = "http2", feature = "tls-rustls"))]
		let allow_http2 = config.http2_builder.is_some();

		#[cfg(feature = "tls-rustls")]
		if let Some(acceptor) = &mut config.acceptor {
			let mut alpn = Vec::new();
			#[cfg(feature = "http2")]
			if allow_http2 {
				alpn.push(b"h2".to_vec());
			}
			#[cfg(feature = "http1")]
			if allow_http1 {
				alpn.push(b"http/1.1".to_vec());
				if allow_http10 {
					alpn.push(b"http/1.0".to_vec());
				}
			}

			acceptor.set_alpn(alpn);
		}

		let threads = listeners
			.into_iter()
			.map(|listener| {
				AbortOnDrop::new(tokio::spawn(
					serve_tcp(listener, service.clone(), config.acceptor.clone(), config.inner())
						.with_context(handler.context()),
				))
			})
			.collect::<Vec<_>>();

		*self.start_group.lock() = Some(StartGroup {
			handler,
			address,
			threads: Arc::new(Mutex::new(threads)),
		});

		Ok(())
	}

	async fn shutdown(&self) -> Result<(), Self::Error> {
		let group = self.start_group.lock().take().ok_or(TcpServerError::NotStarted)?;
		group.handler.cancel();
		group.wait().await?;
		group.handler.shutdown().await;
		Ok(())
	}

	async fn wait(&self) -> Result<(), Self::Error> {
		let group = self.start_group.lock().clone().ok_or(TcpServerError::NotStarted)?;
		group.wait().await
	}

	fn local_addr(&self) -> Result<std::net::SocketAddr, Self::Error> {
		Ok(self.start_group.lock().as_ref().ok_or(TcpServerError::NotStarted)?.address)
	}
}
