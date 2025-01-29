use std::{io, net::SocketAddr};

use crate::server::Server;

#[tracing::instrument(name = "insecure::server", skip_all)]
pub async fn server<M, D>(server: &Server<M>) -> io::Result<()>
where
    M: tower::MakeService<SocketAddr, crate::backend::IncomingRequest, Response = http::Response<D>> + Clone,
    M::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    M::Service: Send + Clone + 'static,
    <M::Service as tower::Service<crate::backend::IncomingRequest>>::Future: Send,
    M::MakeError: std::fmt::Debug,
    M::Future: Send,
    D: http_body::Body + Send + 'static,
    D::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    D::Data: Send,
{
    let Some(bind) = server.config.http.insecure_bind else {
        return Ok(());
    };

    tracing::info!("starting server");

    let listener = tokio::net::TcpListener::bind(bind).await?;

    let mut make_service = server.make_service.clone();

    loop {
        let (tcp_stream, addr) = match listener.accept().await {
            Ok((stream, addr)) => (stream, addr),
            Err(err) => {
                tracing::error!("failed to accept connection: {}", err);
                continue;
            }
        };

        tracing::info!("accepted tcp connection from {}", addr);
        super::handle_connection(&mut make_service, server.config.http.clone(), addr, tcp_stream).await;
    }
}
