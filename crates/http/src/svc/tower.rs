use http::{Request, Response};

use super::ConnectionHandle;
use crate::body::IncomingBody;

#[derive(Debug, Clone, Copy)]
pub struct TowerService<S>(S);

#[async_trait::async_trait]
impl<S, B, E> ConnectionHandle for TowerService<S>
where
	S: tower_service::Service<Request<IncomingBody>, Response = Response<B>, Error = E> + Clone + Send + Sync + 'static,
	S::Future: Send,
	B: http_body::Body + Send + 'static,
	<B as http_body::Body>::Error: Into<crate::Error> + Send + Sync + 'static,
	<B as http_body::Body>::Data: Send,
	E: Into<crate::Error> + Send + Sync + 'static,
{
	type Body = B;
	type BodyData = <B as http_body::Body>::Data;
	type BodyError = <B as http_body::Body>::Error;
	type Error = E;

	async fn on_request(&self, req: Request<IncomingBody>) -> Result<Response<Self::Body>, Self::Error> {
		let mut this = self.0.clone();
		this.call(req).await
	}
}

pub fn tower_service<S>(service: S) -> TowerService<S> {
	TowerService(service)
}
