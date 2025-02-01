use std::fmt::{Debug, Display};
use std::net::SocketAddr;

use hyper_util::rt::{TokioExecutor, TokioIo, TokioTimer};
use hyper_util::server::conn::auto;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::error::Error;
use crate::service::{HttpService, HttpServiceFactory};

pub mod insecure;
pub mod secure;

/// Helper function used by both secure and insecure servers to handle incoming connections.
async fn handle_connection<S, I>(
    service_factory: &mut S,
    addr: SocketAddr,
    io: I,
    http1: bool,
    http2: bool,
) -> Result<(), Error<S>>
where
    S: HttpServiceFactory,
    S::Error: Debug + Display,
    S::Service: Clone + Send + 'static,
    <S::Service as HttpService>::Error: std::error::Error + Debug + Display + Send + Sync,
    <S::Service as HttpService>::ResBody: Send,
    <<S::Service as HttpService>::ResBody as http_body::Body>::Data: Send,
    <<S::Service as HttpService>::ResBody as http_body::Body>::Error: std::error::Error + Send + Sync,
    I: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let io = TokioIo::new(io);

    // make a new service
    let http_service = service_factory.new_service(addr).await.map_err(Error::ServiceFactoryError)?;

    let hyper_proxy_service = hyper::service::service_fn(move |req: http::Request<hyper::body::Incoming>| {
        let mut http_service = http_service.clone();
        async move {
            let (parts, body) = req.into_parts();
            let body = crate::body::IncomingBody::from(body);
            let req = http::Request::from_parts(parts, body);
            http_service.call(req).await
        }
    });

    tokio::spawn(async move {
        let mut builder = auto::Builder::new(TokioExecutor::new());

        let res = if http1 && http2 {
            builder
                .http1()
                .timer(TokioTimer::new())
                .http2()
                .timer(TokioTimer::new())
                .serve_connection_with_upgrades(io, hyper_proxy_service)
                .await
        } else if http1 {
            builder
                .http1_only()
                .serve_connection_with_upgrades(io, hyper_proxy_service)
                .await
        } else if http2 {
            builder
                .http2_only()
                .serve_connection_with_upgrades(io, hyper_proxy_service)
                .await
        } else {
            tracing::warn!("both http1 and http2 are disabled, closing connection");
            return;
        };

        tracing::info!("connection closed: {:?}", res);
    });

    Ok(())
}
