use std::{future::poll_fn, net::SocketAddr};

use hyper_util::{
    rt::{TokioExecutor, TokioIo, TokioTimer},
    server::conn::auto,
};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::config::HttpConfig;

pub mod insecure;
pub mod secure;

/// Helper function used by both secure and insecure servers to handle incoming connections.
async fn handle_connection<M, D, I>(
    make_service: &mut M,
    config: HttpConfig,
    addr: SocketAddr,
    io: I,
) where
    M: tower::MakeService<
        SocketAddr,
        crate::backend::IncomingRequest,
        Response = http::Response<D>,
    >,
    M::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    M::Service: Send + Clone + 'static,
    <M::Service as tower::Service<crate::backend::IncomingRequest>>::Future: Send,
    M::MakeError: std::fmt::Debug,
    M::Future: Send,
    D: http_body::Body + Send + 'static,
    D::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    D::Data: Send,
    I: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let io = TokioIo::new(io);

    // make a new service
    poll_fn(|cx| tower::MakeService::poll_ready(make_service, cx))
        .await
        .unwrap();
    let tower_service = tower::MakeService::make_service(make_service, addr)
        .await
        .unwrap();
    let hyper_proxy_service =
        hyper::service::service_fn(move |req: http::Request<hyper::body::Incoming>| {
            let mut tower_service = tower_service.clone();
            async move {
                let (parts, body) = req.into_parts();
                let body = crate::backend::body::IncomingBody::from(body);
                let req = http::Request::from_parts(parts, body);
                tower::Service::call(&mut tower_service, req).await
            }
        });

    tokio::spawn(async move {
        let mut builder = auto::Builder::new(TokioExecutor::new());

        let res = if config.http1_enabled && config.http2_enabled {
            builder
                .http1()
                .timer(TokioTimer::new())
                .http2()
                .timer(TokioTimer::new())
                .serve_connection(io, hyper_proxy_service)
                .await
        } else if config.http1_enabled {
            builder
                .http1_only()
                .serve_connection(io, hyper_proxy_service)
                .await
        } else if config.http2_enabled {
            builder
                .http2_only()
                .serve_connection(io, hyper_proxy_service)
                .await
        } else {
            unreachable!()
        };

        tracing::info!("connection closed: {:?}", res);
    });
}
