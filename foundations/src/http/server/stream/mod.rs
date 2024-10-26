#[cfg(feature = "http3")]
pub mod quic;
pub mod tcp;
#[cfg(feature = "http-tls")]
pub mod tls;

use std::convert::Infallible;
use std::sync::Arc;

pub use axum::body::Body;
pub use axum::extract::Request;
pub use axum::response::{IntoResponse, Response};
use rand::Rng;

use super::Error;

pub trait ServiceHandler: Clone + Send + Sync + 'static {
	/// Called when the service is ready to accept requests.
	fn on_ready(&self) -> impl std::future::Future<Output = ()> + Send {
		async {}
	}
	/// Called when an error occurs. Some errors may be recoverable, others may
	/// not.
	fn on_error(&self, err: Error) -> impl std::future::Future<Output = ()> + Send {
		let _ = err;
		async {}
	}
	/// Called when the connection is closed.
	fn on_close(&self) -> impl std::future::Future<Output = ()> + Send {
		async {}
	}
	/// Called when the connection is hijacked.
	/// When a connection is hijacked the on_close method will never be called.
	/// And there will be no more requests.
	fn on_hijack(&self) -> impl std::future::Future<Output = ()> + Send {
		async {}
	}
	/// Called when a request is received.
	fn on_request(&self, req: Request) -> impl std::future::Future<Output = impl IntoResponse> + Send;
}

pub trait MakeService: Clone + Send + Sync + 'static {
	fn make_service(
		&self,
		incoming: &impl IncomingConnection,
	) -> impl std::future::Future<Output = Option<impl ServiceHandler>> + Send;
}

pub trait Backend: Sized {
	fn handler(&self) -> &crate::context::Handler;
	fn serve(
		self,
		service: impl MakeService,
	) -> impl std::future::Future<Output = Result<crate::context::Handler, super::Error>> + Send;
}

impl<F, Fut, Resp> ServiceHandler for F
where
	F: tower::Service<axum::extract::Request, Response = Resp, Error = Infallible, Future = Fut>
		+ Clone
		+ Send
		+ Sync
		+ 'static,
	Fut: std::future::Future<Output = Result<Resp, Infallible>> + Send,
	Resp: IntoResponse + Send,
{
	async fn on_request(&self, req: axum::extract::Request) -> impl IntoResponse {
		let resp = self.clone().call(req).await.unwrap_or_else(|err| match err {});
		resp.into_response()
	}
}

#[derive(Clone, Debug)]
pub enum EmptyService {}

impl ServiceHandler for EmptyService {
	async fn on_request(&self, _: axum::extract::Request) -> impl IntoResponse {
		unreachable!("EmptyService::on_request should never be called");
		#[allow(unreachable_code)]
		Response::new(Body::empty())
	}
}

#[derive(Clone, Debug)]
pub struct EmptyMakeService;

impl MakeService for EmptyMakeService {
	async fn make_service(&self, _: &impl IncomingConnection) -> Option<impl ServiceHandler> {
		None::<EmptyService>
	}
}

#[derive(Clone, Debug)]
pub struct TowerService<S>(pub S);

impl<F, Fut, Resp> MakeService for TowerService<F>
where
	F: tower::Service<axum::extract::Request, Response = Resp, Error = Infallible, Future = Fut>
		+ Clone
		+ Send
		+ Sync
		+ 'static,
	Fut: std::future::Future<Output = Result<Resp, Infallible>> + Send,
	Resp: IntoResponse + Send,
{
	async fn make_service(&self, _: &impl IncomingConnection) -> Option<impl ServiceHandler> {
		Some(self.0.clone())
	}
}

impl<M, S, Fut, Resp> MakeService for M
where
	M: for<'a> tower::Service<&'a dyn IncomingConnection, Error = Infallible, Response = Option<S>, Future = Fut>
		+ Send
		+ Clone
		+ Sync
		+ 'static,
	S: tower::Service<axum::extract::Request, Response = Resp, Error = Infallible> + Clone + Send + Sync + 'static,
	Fut: std::future::Future<Output = Result<Option<S>, Infallible>> + Send,
	S::Future: Send,
	Resp: IntoResponse + Send,
{
	async fn make_service(&self, incoming: &impl IncomingConnection) -> Option<impl ServiceHandler> {
		self.clone().call(incoming).await.unwrap_or_else(|err| match err {})
	}
}

impl MakeService for axum::routing::Router<()> {
	fn make_service(
		&self,
		_: &impl IncomingConnection,
	) -> impl std::future::Future<Output = Option<impl ServiceHandler>> + Send {
		std::future::ready(Some(self.clone()))
	}
}

pub trait IncomingConnection: Send + Sync {
	fn socket_kind(&self) -> SocketKind;
	fn remote_addr(&self) -> std::net::SocketAddr;
	fn local_addr(&self) -> Option<std::net::SocketAddr>;
	fn is_encrypted(&self) -> bool {
		matches!(self.socket_kind(), SocketKind::TlsTcp | SocketKind::Quic)
	}

	fn downcast<T: 'static>(&self) -> Option<&T>
	where
		Self: Sized,
	{
		None
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SocketKind {
	Tcp,
	TlsTcp,
	Quic,
}

fn jitter(duration: std::time::Duration) -> std::time::Duration {
	let mut rng = rand::thread_rng();
	let jitter = rng.gen_range(0..duration.as_millis() / 10);
	duration + std::time::Duration::from_millis(jitter as u64)
}

struct ActiveRequestsGuard(Arc<std::sync::atomic::AtomicUsize>);

impl Drop for ActiveRequestsGuard {
	fn drop(&mut self) {
		self.0.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
	}
}

impl ActiveRequestsGuard {
	fn new(active_requests: Arc<std::sync::atomic::AtomicUsize>) -> Self {
		active_requests.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
		Self(active_requests)
	}
}

fn is_fatal_tcp_error(err: &std::io::Error) -> bool {
	matches!(
		err.raw_os_error(),
		Some(libc::EFAULT)
			| Some(libc::EINVAL)
			| Some(libc::ENFILE)
			| Some(libc::EMFILE)
			| Some(libc::ENOBUFS)
			| Some(libc::ENOMEM)
	)
}
