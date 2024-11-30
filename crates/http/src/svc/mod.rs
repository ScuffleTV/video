use std::net::SocketAddr;

use crate::body::IncomingBody;

#[cfg(feature = "axum")]
mod axum;
mod function;
#[cfg(feature = "tower")]
mod tower;

#[cfg(feature = "axum")]
pub use axum::{axum_service, AxumService};
pub use function::{function_service, FunctionService};
#[cfg(feature = "tower")]
pub use tower::{tower_service, TowerService};

#[cfg(feature = "tracing")]
use crate::error::ErrorSeverity;

#[async_trait::async_trait]
pub trait ConnectionHandle: Send + Sync + 'static {
	type Body: http_body::Body<Error = Self::BodyError, Data = Self::BodyData> + Send + 'static;
	type BodyData: bytes::Buf + Send;
	type BodyError: Into<crate::Error> + Send + Sync;
	type Error: Into<crate::Error> + Send;

	/// After the previous accept call has returned, we call this method to
	/// allow for a deferred accept in an async context, returning an error
	/// here will reject the connection before any work is done (such as a
	/// TLS handshake).
	async fn accept(&self, conn: IncomingConnection) -> Result<(), Self::Error> {
		let _ = conn;
		Ok(())
	}

	/// The `on_request` method is called when a new request is received.
	/// You need to provide a `RequestHandle` that will be used to handle the
	/// request. You are given a request so you can inspect it and decide if
	/// you want to handle it or not. If you return an Error, the connection
	/// will be closed.
	async fn on_request(&self, req: http::Request<IncomingBody>) -> Result<http::Response<Self::Body>, Self::Error>;

	/// The `on_ready` method is called when the connection is ready to be used.
	/// This is after all protocol negotiation has been performed and we are now
	/// ready to process http requests.
	fn on_ready(&self) {}

	/// The `on_close` method is called when the connection is closed.
	/// This is called after all requests have completed and the connection has
	/// closed.
	fn on_close(&self) {}

	/// The `on_error` method is called when an error occurs on the connection.
	fn on_error(&self, err: crate::Error) {
		#[cfg(feature = "tracing")]
		match err.severity() {
			ErrorSeverity::Warning => tracing::warn!("{err}"),
			ErrorSeverity::Info => tracing::info!("{err}"),
			ErrorSeverity::Debug => tracing::debug!("{err}"),
			_ => tracing::error!("{err}"),
		}

		#[cfg(not(feature = "tracing"))]
		let _ = err;
	}
}

/// A struct representing an incoming connection.
pub struct IncomingConnection {
	/// The address the connection is coming from.
	pub addr: SocketAddr,
}

pub trait ConnectionAcceptor: Send + Sync + 'static {
	type Handle: ConnectionHandle;

	/// The `accept` method is called when a new connection is attempting to be
	/// accepted. This is before any tls handshake or other protocol
	/// negotiation has been performed. It is useful to be able to reject
	/// connections before any work is done. This method also blocks accepting
	/// other connections until it returns, so you should not do any blocking
	/// work here.
	fn accept(&self, conn: IncomingConnection) -> Option<Self::Handle>;
}

#[async_trait::async_trait]
impl<T: ConnectionHandle + Clone + Sync> ConnectionAcceptor for T {
	type Handle = T;

	fn accept(&self, _: IncomingConnection) -> Option<Self::Handle> {
		Some(self.clone())
	}
}
