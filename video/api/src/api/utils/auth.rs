use std::str::FromStr;
use std::sync::Arc;

use base64::Engine;
use futures_util::future::BoxFuture;
use tonic::body::BoxBody;
use tonic::Status;
use ulid::Ulid;
use video_common::database::AccessToken;

use super::{AccessTokenExt, RequiredScope};
use crate::global::ApiGlobal;

pub struct AuthMiddleware<G>(std::marker::PhantomData<G>);

impl<G> Default for AuthMiddleware<G> {
	fn default() -> Self {
		Self(std::marker::PhantomData)
	}
}

impl<G> Clone for AuthMiddleware<G> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<G> Copy for AuthMiddleware<G> {}

impl<G> std::fmt::Debug for AuthMiddleware<G> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("AuthMiddleware").finish()
	}
}

impl<S, G> tower::Layer<S> for AuthMiddleware<G> {
	type Service = AuthSvc<S, G>;

	fn layer(&self, inner: S) -> Self::Service {
		AuthSvc {
			inner,
			_marker: std::marker::PhantomData,
		}
	}
}

pub struct AuthSvc<S, G> {
	inner: S,
	_marker: std::marker::PhantomData<G>,
}

impl<S, G> Clone for AuthSvc<S, G>
where
	S: Clone,
{
	fn clone(&self) -> Self {
		Self {
			inner: self.inner.clone(),
			_marker: self._marker,
		}
	}
}

impl<S: Copy, G> Copy for AuthSvc<S, G> {}

impl<S, G> std::fmt::Debug for AuthSvc<S, G>
where
	S: std::fmt::Debug,
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("AuthService").field("inner", &self.inner).finish()
	}
}

impl<S, G> tower::Service<http::Request<hyper::Body>> for AuthSvc<S, G>
where
	S: tower::Service<http::Request<hyper::Body>, Response = http::Response<BoxBody>> + Clone + Send + 'static,
	G: ApiGlobal,
	S::Error: From<Status>,
	S::Future: Send + 'static,
{
	type Error = S::Error;
	type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;
	type Response = S::Response;

	fn call(&mut self, req: http::Request<hyper::Body>) -> Self::Future {
		let mut inner = self.inner.clone();

		Box::pin(async move {
			let req = auth_middleware_impl::<G>(req).await?;

			inner.call(req).await
		})
	}

	fn poll_ready(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
		self.inner.poll_ready(cx)
	}
}

pub async fn auth_middleware_impl<G: ApiGlobal>(
	mut req: http::Request<hyper::Body>,
) -> tonic::Result<http::Request<hyper::Body>> {
	let global = req
		.extensions()
		.get::<Arc<G>>()
		.ok_or_else(|| tonic::Status::internal("global state missing"))?;

	let organization_id = req
		.headers()
		.get("x-scuffle-organization-id")
		.ok_or_else(|| Status::unauthenticated("no organization id header"))?
		.to_str()
		.ok()
		.and_then(|s| Ulid::from_str(s).ok())
		.ok_or_else(|| Status::unauthenticated("invalid organization id header"))?;

	let (access_token_id, secret_key) = req
		.headers()
		.get(hyper::header::AUTHORIZATION)
		.ok_or_else(|| Status::unauthenticated("no authorization header"))?
		.to_str()
		.ok()
		.and_then(|s| s.strip_prefix("Basic "))
		.and_then(|s| {
			let s = base64::engine::general_purpose::URL_SAFE.decode(s.as_bytes()).ok()?;
			let s = std::str::from_utf8(&s).ok()?;

			let mut parts = s.splitn(2, ':');

			let access_token_id = Ulid::from_str(parts.next()?).ok()?;
			let secret_key = Ulid::from_str(parts.next()?).ok()?;

			Some((access_token_id, secret_key))
		})
		.ok_or_else(|| Status::unauthenticated("invalid authorization header"))?;

	let access_token = global
		.access_token_loader()
		.load((organization_id, access_token_id))
		.await
		.map_err(|()| Status::internal("failed to load access token"))?
		.ok_or_else(|| Status::unauthenticated("invalid access token"))?;

	if let Some(expires_at) = &access_token.expires_at {
		let now = chrono::Utc::now();

		if expires_at < &now {
			return Err(Status::unauthenticated("invalid access token"));
		}
	}

	if access_token.secret_token != secret_key {
		return Err(Status::unauthenticated("invalid access token"));
	}

	req.extensions_mut().insert(access_token);

	Ok(req)
}

pub fn validate_request<'a, T>(
	req: &'a tonic::Request<T>,
	required_scope: &RequiredScope,
) -> Result<&'a AccessToken, Status> {
	let access_token = req
		.extensions()
		.get::<AccessToken>()
		.ok_or_else(|| Status::internal("access token missing"))?;

	access_token.has_scope(required_scope)?;

	Ok(access_token)
}
