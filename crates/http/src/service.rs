use std::future::Future;
use std::net::SocketAddr;

use crate::backend::IncomingRequest;

pub trait HttpService {
    type Error;
    type ResBody: http_body::Body;

    fn call(
        &mut self,
        req: IncomingRequest,
    ) -> impl Future<Output = Result<http::Response<Self::ResBody>, Self::Error>> + Send;
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
