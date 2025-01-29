use std::{io, net::SocketAddr, sync::Arc};

use crate::server::Server;

#[tracing::instrument(name = "secure::server", skip_all)]
pub async fn server<M, B>(
    server: &Server<M>,
) -> io::Result<()>
where
    M: tower::MakeService<
        SocketAddr,
        crate::backend::IncomingRequest,
        Response = http::Response<B>,
    > + Clone,
    M::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    M::Service: Send + Clone + 'static,
    <M::Service as tower::Service<crate::backend::IncomingRequest>>::Future: Send,
    M::MakeError: std::fmt::Debug,
    M::Future: Send,
    B: http_body::Body + Send + 'static,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    B::Data: Send,
{
    let Some(bind) = server.config.http.secure_bind else {
        // TODO: return error
        return Ok(());
    };

    tracing::info!("starting server");

    let Some(mut server_config) = server.tls_config() else {
        return Ok(());
    };
    // reset it back to 0 because everything explodes if it's not
    // https://github.com/hyperium/hyper/issues/3841
    server_config.max_early_data_size = 0;

    let listener = tokio::net::TcpListener::bind(bind).await?;
    let tls_acceptor = tokio_rustls::TlsAcceptor::from(Arc::new(server_config));

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

        let stream = match tls_acceptor.accept(tcp_stream).await {
            Ok(stream) => stream,
            Err(err) => {
                tracing::error!("failed to accept TLS connection: {}", err);
                continue;
            }
        };

        tracing::info!("accepted tls connection from {}", addr);
        super::handle_connection(&mut make_service, server.config.http.clone(), addr, stream).await;
    }
}
