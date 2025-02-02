use std::future::Future;
use std::net::SocketAddr;

use crate::IncomingRequest;

mod clone_factory;
mod function;
mod tower_factory;

pub use clone_factory::*;
pub use function::*;
pub use tower_factory::*;

/// A trait representing an HTTP service.
///
/// This trait must be used in combination with [`HttpServiceFactory`].
/// It is very similar to tower's service trait and implemented
/// for all types that implement [`tower::Service<IncomingRequest>`](https://docs.rs/tower/latest/tower/trait.Service.html).
pub trait HttpService {
    type Error;
    type ResBody: http_body::Body;

    /// Handle an incoming request.
    ///
    /// This method is called for each incoming request.
    /// The service must return a response for the given request.
    fn call(
        &mut self,
        req: IncomingRequest,
    ) -> impl Future<Output = Result<http::Response<Self::ResBody>, Self::Error>> + Send;
}

// Implement for tower services
impl<T, B> HttpService for T
where
    T: tower::Service<IncomingRequest, Response = http::Response<B>> + Send,
    T::Future: Send,
    B: http_body::Body,
{
    type Error = T::Error;
    type ResBody = B;

    async fn call(&mut self, req: IncomingRequest) -> Result<http::Response<Self::ResBody>, Self::Error> {
        // wait for the tower service to be ready
        futures::future::poll_fn(|cx| self.poll_ready(cx)).await?;

        self.call(req).await
    }
}

/// A trait representing an HTTP service factory.
///
/// This trait must be implemented by types that can create new instances of [`HttpService`].
/// It is conceptually similar to tower's [`MakeService`](https://docs.rs/tower/latest/tower/trait.MakeService.html) trait.
///
/// It is intended to create a new service for each incoming connection.
/// If you don't need to implement any custom factory logic, you can use [`ServiceCloneFactory`] to make a factory that clones the given service for each new connection.
pub trait HttpServiceFactory {
    type Error;
    type Service: HttpService;

    /// Create a new service for a new connection.
    ///
    /// `remote_addr` is the address of the connecting remote peer.
    fn new_service(&mut self, remote_addr: SocketAddr) -> impl Future<Output = Result<Self::Service, Self::Error>> + Send;
}
