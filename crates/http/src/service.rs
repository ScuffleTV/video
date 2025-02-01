use std::fmt::Debug;
use std::future::Future;
use std::net::SocketAddr;

use crate::IncomingRequest;

pub trait HttpService {
    type Error;
    type ResBody: http_body::Body;

    fn call(
        &mut self,
        req: IncomingRequest,
    ) -> impl Future<Output = Result<http::Response<Self::ResBody>, Self::Error>> + Send;
}

#[derive(Clone)]
pub struct FnHttpService<F>(F);

impl<F> Debug for FnHttpService<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("FnHttpService").field(&std::any::type_name::<F>()).finish()
    }
}

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

impl<T, B> HttpService for T
where
    T: tower::Service<IncomingRequest, Response = http::Response<B>> + Send,
    T::Future: Send,
    B: http_body::Body,
{
    type Error = T::Error;
    type ResBody = B;

    async fn call(&mut self, req: IncomingRequest) -> Result<http::Response<Self::ResBody>, Self::Error> {
        // wait for the service to be ready
        futures::future::poll_fn(|cx| self.poll_ready(cx)).await?;

        self.call(req).await
    }
}

pub trait HttpServiceFactory {
    type Error;
    type Service: HttpService;

    fn new_service(&mut self, remote_addr: SocketAddr) -> impl Future<Output = Result<Self::Service, Self::Error>> + Send;
}

#[derive(Clone)]
pub struct FnHttpServiceFactory<F>(F);

impl<F> Debug for FnHttpServiceFactory<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("FnHttpServiceFactory")
            .field(&std::any::type_name::<F>())
            .finish()
    }
}

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

impl<T> HttpServiceFactory for T
where
    T: tower::MakeService<SocketAddr, IncomingRequest> + Send,
    T::Future: Send,
    T::Service: HttpService,
{
    type Error = T::MakeError;
    type Service = T::Service;

    async fn new_service(&mut self, remote_addr: SocketAddr) -> Result<Self::Service, Self::Error> {
        // wait for the service to be ready
        futures::future::poll_fn(|cx| self.poll_ready(cx)).await?;

        self.make_service(remote_addr).await
    }
}
