use std::{future::Future, net::SocketAddr};

use crate::backend::IncomingRequest;

pub trait HttpService {
    type Error;
    type ResBody: http_body::Body;

    fn call(&mut self, req: IncomingRequest) -> impl Future<Output = Result<http::Response<Self::ResBody>, Self::Error>>;
}

impl<T, B> HttpService for T
where
    T: tower::Service<IncomingRequest, Response = http::Response<B>>,
    B: http_body::Body,
{
    type Error = T::Error;
    type ResBody = B;

    fn call(&mut self, req: IncomingRequest) -> impl Future<Output = Result<http::Response<Self::ResBody>, Self::Error>> {
        self.call(req)
    }
}

pub trait HttpServiceFactory {
    type Error;
    type Service: HttpService;

    fn new_service(&mut self, addr: SocketAddr) -> impl Future<Output = Result<Self::Service, Self::Error>>;
}

impl<T> HttpServiceFactory for T
where
    T: tower::MakeService<SocketAddr, IncomingRequest>,
    T::Service: HttpService,
{
    type Error = T::MakeError;
    type Service = T::Service;

    fn new_service(&mut self, addr: SocketAddr) -> impl Future<Output = Result<Self::Service, Self::Error>> {
        self.make_service(addr)
    }
}
