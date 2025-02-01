use std::net::SocketAddr;

use super::{HttpService, HttpServiceFactory};

#[derive(Clone, Debug)]
pub struct ServiceCloneFactory<S>(S);

pub fn service_clone_factory<S>(service: S) -> ServiceCloneFactory<S>
where
    S: HttpService + Clone + Send,
{
    ServiceCloneFactory(service)
}

impl<S> HttpServiceFactory for ServiceCloneFactory<S>
where
    S: HttpService + Clone + Send,
{
    type Error = std::convert::Infallible;
    type Service = S;

    async fn new_service(&mut self, _remote_addr: SocketAddr) -> Result<Self::Service, Self::Error> {
        Ok(self.0.clone())
    }
}
