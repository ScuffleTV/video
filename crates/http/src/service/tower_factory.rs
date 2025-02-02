use std::net::SocketAddr;

use super::{HttpService, HttpServiceFactory};
use crate::IncomingRequest;

/// A [`HttpServiceFactory`] that wraps a [`tower::MakeService`].
/// The given [`tower::MakeService`] will be called to create a new service for each new connection.
///
/// Create by calling [`tower_make_service_factory`] or [`custom_tower_make_service_factory`].
#[derive(Clone, Debug)]
pub struct TowerMakeServiceFactory<M, T> {
    make_service: M,
    target: T,
}

/// Create a [`TowerMakeServiceFactory`] from a given [`tower::MakeService`] and `target` value.
///
/// `target` is the value that will be passed to the [`tower::MakeService::make_service`] method.
/// `target` will be cloned for each new connection.
/// If the `target` should be the remote address of the incoming connection, use [`tower_make_service_with_addr_factory`] instead.
/// If `target` is not needed, use [`tower_make_service_factory`] instead.
pub fn custom_tower_make_service_factory<M, T>(make_service: M, target: T) -> TowerMakeServiceFactory<M, T> {
    TowerMakeServiceFactory { make_service, target }
}

/// Create a [`TowerMakeServiceFactory`] from a given [`tower::MakeService`].
///
/// Can be used with [`axum::Router::into_make_service`](https://docs.rs/axum/latest/axum/struct.Router.html#method.into_make_service).
pub fn tower_make_service_factory<M>(make_service: M) -> TowerMakeServiceFactory<M, ()> {
    TowerMakeServiceFactory {
        make_service,
        target: (),
    }
}

impl<M, T> HttpServiceFactory for TowerMakeServiceFactory<M, T>
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

/// A [`HttpServiceFactory`] that wraps a [`tower::MakeService`] that takes a [`SocketAddr`] as input.
///
/// Can be used with [`axum::Router::into_make_service_with_connect_info`](https://docs.rs/axum/latest/axum/struct.Router.html#method.into_make_service_with_connect_info).
#[derive(Clone, Debug)]
pub struct TowerMakeServiceWithAddrFactory<M>(M);

/// Create a [`TowerMakeServiceWithAddrFactory`] from a given [`tower::MakeService`].
///
/// See [`TowerMakeServiceFactory`] for details.
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
