use std::fmt::Debug;
use std::future::Future;
use std::net::SocketAddr;

use super::{HttpService, HttpServiceFactory};
use crate::IncomingRequest;

/// A [`HttpService`] that is created from a function.
///
/// The given function will be called for each incoming request.
/// This is useful for creating simple services without needing to implement the [`HttpService`] trait.
///
/// Create by calling [`fn_http_service`].
#[derive(Clone)]
pub struct FnHttpService<F>(F);

impl<F> Debug for FnHttpService<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("FnHttpService").field(&std::any::type_name::<F>()).finish()
    }
}

/// Create a [`FnHttpService`] from a given function.
///
/// See [`FnHttpService`] for details.
pub fn fn_http_service<F, Fut, E, B>(f: F) -> FnHttpService<F>
where
    F: Fn(IncomingRequest) -> Fut,
    Fut: Future<Output = Result<http::Response<B>, E>> + Send,
    E: std::error::Error,
    B: http_body::Body,
{
    FnHttpService(f)
}

impl<F, Fut, E, B> HttpService for FnHttpService<F>
where
    F: Fn(IncomingRequest) -> Fut,
    Fut: Future<Output = Result<http::Response<B>, E>> + Send,
    E: std::error::Error,
    B: http_body::Body,
{
    type Error = E;
    type ResBody = B;

    fn call(
        &mut self,
        req: IncomingRequest,
    ) -> impl Future<Output = Result<http::Response<Self::ResBody>, Self::Error>> + Send {
        (self.0)(req)
    }
}

/// A [`HttpServiceFactory`] that creates a [`FnHttpService`] from a function.
///
/// The given function will be called for each new connection.
/// This is useful for creating simple factories without needing to implement the [`HttpServiceFactory`] trait.
///
/// Create by calling [`fn_http_service_factory`].
#[derive(Clone)]
pub struct FnHttpServiceFactory<F>(F);

impl<F> Debug for FnHttpServiceFactory<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("FnHttpServiceFactory")
            .field(&std::any::type_name::<F>())
            .finish()
    }
}

/// Create a [`FnHttpServiceFactory`] from a given function.
///
/// See [`FnHttpServiceFactory`] for details.
pub fn fn_http_service_factory<F, Fut, E, S>(f: F) -> FnHttpServiceFactory<F>
where
    F: Fn(SocketAddr) -> Fut,
    Fut: Future<Output = Result<S, E>> + Send,
    E: std::error::Error,
    S: HttpService,
{
    FnHttpServiceFactory(f)
}

impl<F, Fut, E, S> HttpServiceFactory for FnHttpServiceFactory<F>
where
    F: Fn(SocketAddr) -> Fut,
    Fut: Future<Output = Result<S, E>> + Send,
    E: std::error::Error,
    S: HttpService,
{
    type Error = E;
    type Service = S;

    fn new_service(&mut self, remote_addr: SocketAddr) -> impl Future<Output = Result<Self::Service, Self::Error>> + Send {
        (self.0)(remote_addr)
    }
}
