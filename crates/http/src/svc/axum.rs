use std::convert::Infallible;

use super::ConnectionHandle;
use crate::body::IncomingBody;

/// A wrapper to allow using a Axum style Tower Service to implement an HTTP
/// Service.
#[derive(Debug, Clone, Copy)]
pub struct AxumService<S>(S);

#[async_trait::async_trait]
impl<S, E> ConnectionHandle for AxumService<S>
where
	S: tower_service::Service<
			http::Request<axum_core::body::Body>,
			Response = http::Response<axum_core::body::Body>,
			Error = E,
		> + Clone
		+ Send
		+ Sync
		+ 'static,
	S::Future: Send,
	E: axum_core::response::IntoResponse + Send + Sync,
{
	type Body = axum_core::body::Body;
	type BodyData = <axum_core::body::Body as http_body::Body>::Data;
	type BodyError = axum_core::Error;
	type Error = Infallible;

	async fn on_request(&self, req: http::Request<IncomingBody>) -> Result<http::Response<Self::Body>, Self::Error> {
		let mut this = self.0.clone();
		match this.call(req.map(axum_core::body::Body::new)).await {
			Ok(res) => Ok(res),
			Err(e) => Ok(e.into_response()),
		}
	}
}

pub fn axum_service<S>(service: S) -> AxumService<S> {
	AxumService(service)
}
