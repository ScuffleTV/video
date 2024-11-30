pub mod config;
mod serve;
use std::sync::Arc;

pub use config::QuinnServerConfig;
use serve::serve_quinn;
use tokio::sync::Mutex;

use crate::backend::HttpServer;
use crate::svc::ConnectionAcceptor;
use crate::util::AbortOnDrop;

#[derive(Debug, thiserror::Error)]
pub enum QuinnServerError {
	#[error("not implemented")]
	NotImplemented,
	#[error("the server has already been started")]
	AlreadyStarted,
	#[error("io: {0}")]
	Io(#[from] std::io::Error),
	#[error("the server has not been started")]
	NotStarted,
	#[error(transparent)]
	SharedError(#[from] Arc<QuinnServerError>),
	#[error(transparent)]
	JoinError(#[from] tokio::task::JoinError),
}

#[derive(Clone)]
struct StartGroup {
	handler: scuffle_context::Handler,
	address: std::net::SocketAddr,
	#[allow(clippy::type_complexity)]
	threads: Arc<Mutex<Vec<AbortOnDrop<Result<(), QuinnServerError>>>>>,
}

impl StartGroup {
	async fn wait(&self) -> Result<(), QuinnServerError> {
		let mut threads = self.threads.lock().await;

		while !threads.is_empty() {
			let (result, _, remaining) = futures::future::select_all(threads.drain(..).map(|thread| thread.disarm())).await;
			*threads = remaining.into_iter().map(AbortOnDrop::new).collect();
			result??;
		}

		Ok(())
	}
}

pub struct QuinnServer {
	config: tokio::sync::Mutex<QuinnServerConfig>,
	start_group: spin::Mutex<Option<StartGroup>>,
}

impl QuinnServer {
	pub fn new(config: QuinnServerConfig) -> Self {
		Self {
			config: tokio::sync::Mutex::new(config),
			start_group: spin::Mutex::new(None),
		}
	}
}

impl HttpServer for QuinnServer {
	type Error = QuinnServerError;

	async fn start<S: ConnectionAcceptor + Clone>(&self, service: S, workers: usize) -> Result<(), Self::Error> {
		let mut config = self.config.lock().await;

		let mut group = self.start_group.lock().take();
		if let Some(group) = group.take() {
			group.handler.cancel();
		}

		let listener = config.make_listener.make()?;

		let address = listener.local_addr()?;

		let listeners = (0..workers).map(|_| listener.try_clone()).collect::<Result<Vec<_>, _>>()?;

		let endpoints = listeners
			.into_iter()
			.map(|listener| {
				quinn::Endpoint::new(
					config.endpoint_config.clone(),
					Some(config.quinn_config.clone()),
					listener,
					Arc::new(quinn::TokioRuntime),
				)
			})
			.collect::<Result<Vec<_>, _>>()?;

		let handler = scuffle_context::Handler::new();

		let inner_config = Arc::new(config.inner());

		let threads = endpoints
			.into_iter()
			.map(|endpoint| {
				AbortOnDrop::new(tokio::spawn(serve_quinn(
					endpoint,
					service.clone(),
					inner_config.clone(),
					handler.context(),
				)))
			})
			.collect::<Vec<_>>();

		*self.start_group.lock() = Some(StartGroup {
			handler,
			address,
			threads: Arc::new(Mutex::new(threads)),
		});

		Ok(())
	}

	async fn wait(&self) -> Result<(), Self::Error> {
		let start_group = self.start_group.lock().clone().ok_or(QuinnServerError::NotStarted)?;
		start_group.wait().await
	}

	async fn shutdown(&self) -> Result<(), Self::Error> {
		let start_group = self.start_group.lock().take().ok_or(QuinnServerError::NotStarted)?;
		start_group.handler.cancel();
		start_group.wait().await?;
		start_group.handler.shutdown().await;
		Ok(())
	}

	fn local_addr(&self) -> Result<std::net::SocketAddr, Self::Error> {
		Ok(self.start_group.lock().as_ref().ok_or(QuinnServerError::NotStarted)?.address)
	}
}
