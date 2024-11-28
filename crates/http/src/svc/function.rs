use std::future::Future;

use http::{Request, Response};

use super::ConnectionHandle;
use crate::body::IncomingBody;

#[derive(Debug, Clone, Copy)]
pub struct FunctionService<S>(S);

#[async_trait::async_trait]
impl<S, B, F, E> ConnectionHandle for FunctionService<S>
where
	S: Fn(Request<IncomingBody>) -> F + Send + Sync + 'static,
	F: Future<Output = Result<Response<B>, E>> + Send + 'static,
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
		(self.0)(req).await
	}
}

pub fn function_service<S, F, B, E>(service: S) -> FunctionService<S>
where
	S: Fn(Request<IncomingBody>) -> F + Send + Sync + 'static,
	F: Future<Output = Result<Response<B>, E>> + Send + 'static,
	B: http_body::Body + Send + 'static,
	<B as http_body::Body>::Error: Into<crate::Error> + Send + Sync + 'static,
	<B as http_body::Body>::Data: Send,
	E: Into<crate::Error> + Send + Sync + 'static,
{
	FunctionService(service)
}
