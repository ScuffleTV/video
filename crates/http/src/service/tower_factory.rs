use std::net::SocketAddr;

use crate::IncomingRequest;

use super::{HttpService, HttpServiceFactory};

#[derive(Clone, Debug)]
pub struct TowerMakeServiceFactory<M>(M);

pub fn tower_make_service_factory<M>(make_service: M) -> TowerMakeServiceFactory<M> {
    TowerMakeServiceFactory(make_service)
}

impl<M> HttpServiceFactory for TowerMakeServiceFactory<M>
where
    M: tower::MakeService<(), IncomingRequest> + Send,
    M::Future: Send,
    M::Service: HttpService,
{
    type Error = M::MakeError;
    type Service = M::Service;

    async fn new_service(&mut self, _remote_addr: SocketAddr) -> Result<Self::Service, Self::Error> {
        // wait for the service to be ready
        futures::future::poll_fn(|cx| self.0.poll_ready(cx)).await?;

        self.0.make_service(()).await
    }
}

#[derive(Clone, Debug)]
pub struct TowerMakeServiceWithAddrFactory<M>(M);

pub fn tower_make_service_with_addr_factory<M>(make_service: M) -> TowerMakeServiceWithAddrFactory<M> {
    TowerMakeServiceWithAddrFactory(make_service)
}

impl<M> HttpServiceFactory for TowerMakeServiceWithAddrFactory<M>
where
    M: tower::MakeService<SocketAddr, IncomingRequest> + Send,
    M::Future: Send,
    M::Service: HttpService,
{
    type Error = M::MakeError;
    type Service = M::Service;

    async fn new_service(&mut self, remote_addr: SocketAddr) -> Result<Self::Service, Self::Error> {
        // wait for the service to be ready
        futures::future::poll_fn(|cx| self.0.poll_ready(cx)).await?;

        self.0.make_service(remote_addr).await
    }
}

#[derive(Clone, Debug)]
pub struct CustomTowerMakeServiceFactory<M, T> {
    make_service: M,
    target: T,
}

pub fn custom_tower_make_service_factory<M, T>(make_service: M, target: T) -> CustomTowerMakeServiceFactory<M, T> {
    CustomTowerMakeServiceFactory {
        make_service,
        target,
    }
}

impl<M, T> HttpServiceFactory for CustomTowerMakeServiceFactory<M, T>
where
    M: tower::MakeService<T, IncomingRequest> + Send,
    M::Future: Send,
    M::Service: HttpService,
    T: Clone + Send,
{
    type Error = M::MakeError;
    type Service = M::Service;

    async fn new_service(&mut self, _remote_addr: SocketAddr) -> Result<Self::Service, Self::Error> {
        // wait for the service to be ready
        futures::future::poll_fn(|cx| self.make_service.poll_ready(cx)).await?;

        self.make_service.make_service(self.target.clone()).await
    }
}
