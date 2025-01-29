use std::{future::poll_fn, io, net::SocketAddr, sync::Arc};

use h3_quinn::BidiStream;

use body::QuicIncomingBody;
use utils::copy_response_body;

pub mod body;
mod utils;

#[derive(Debug, Clone)]
pub struct Http3Backend {
    pub bind: SocketAddr,
}

impl Default for Http3Backend {
    fn default() -> Self {
        Self {
            bind: "[::]:443".parse().unwrap(),
        }
    }
}

impl Http3Backend {
    pub fn alpn_protocols(&self) -> Vec<Vec<u8>> {
        vec![b"h3".to_vec()]
    }

    pub async fn run<M, D>(self, make_service: M, mut rustls_config: rustls::ServerConfig) -> io::Result<()>
    where
        M: tower::MakeService<SocketAddr, crate::backend::IncomingRequest, Response = hyper::Response<D>>
            + Clone
            + Send
            + 'static,
        M::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
        M::Service: Send + Clone + 'static,
        <M::Service as tower::Service<crate::backend::IncomingRequest>>::Future: Send,
        M::MakeError: std::fmt::Debug,
        M::Future: Send,
        D: http_body::Body + Send + 'static,
        D::Error: Into<Box<dyn std::error::Error + Send + Sync>> + Send,
        D::Data: Send,
    {
        tracing::info!("starting server");

        // not quite sure why this is necessary but it is
        rustls_config.max_early_data_size = u32::MAX;
        let crypto = h3_quinn::quinn::crypto::rustls::QuicServerConfig::try_from(rustls_config).unwrap();
        let server_config = h3_quinn::quinn::ServerConfig::with_crypto(Arc::new(crypto));

        let endpoint = h3_quinn::quinn::Endpoint::server(server_config, "0.0.0.0:443".parse().unwrap())?;

        // handle incoming connections and requests
        while let Some(new_conn) = endpoint.accept().await {
            tracing::info!("new connection being attempted");

            let mut make_service = make_service.clone();

            tokio::spawn(async move {
                match new_conn.await {
                    Ok(conn) => {
                        tracing::info!("new connection established");

                        let addr = conn.remote_address();

                        let mut h3_conn = h3::server::Connection::new(h3_quinn::Connection::new(conn)).await.unwrap();

                        loop {
                            match h3_conn.accept().await {
                                Ok(Some((req, stream))) => {
                                    let (mut send, recv) = stream.split();

                                    let size_hint = req
                                        .headers()
                                        .get(hyper::http::header::CONTENT_LENGTH)
                                        .and_then(|len| len.to_str().ok().and_then(|x| x.parse().ok()));
                                    let body: QuicIncomingBody<BidiStream<_>> = QuicIncomingBody::new(recv, size_hint);
                                    let req = http::Request::from_parts(
                                        req.into_parts().0,
                                        crate::backend::body::IncomingBody::from(body),
                                    );

                                    // make a new service
                                    poll_fn(|cx| tower::MakeService::poll_ready(&mut make_service, cx))
                                        .await
                                        .unwrap();
                                    let mut tower_service =
                                        tower::MakeService::make_service(&mut make_service, addr).await.unwrap();

                                    tokio::spawn(async move {
                                        let resp = match tower::Service::call(&mut tower_service, req).await {
                                            Ok(res) => res,
                                            Err(err) => {
                                                tracing::error!("error processing request: {}", err.into());
                                                return;
                                            }
                                        };
                                        let (parts, body) = resp.into_parts();

                                        send.send_response(hyper::Response::from_parts(parts, ())).await.unwrap();

                                        copy_response_body(send, body).await;
                                    });
                                }
                                // indicating no more streams to be received
                                Ok(None) => {
                                    break;
                                }
                                Err(err) => {
                                    tracing::error!("error ({:?}) on accept: {}", err.get_error_level(), err);
                                    match err.get_error_level() {
                                        h3::error::ErrorLevel::ConnectionError => break,
                                        h3::error::ErrorLevel::StreamError => continue,
                                    }
                                }
                            }
                        }
                    }
                    Err(err) => {
                        tracing::error!("accepting connection failed: {:?}", err);
                    }
                }
            });
        }

        // shut down gracefully
        // wait for connections to be closed before exiting
        endpoint.wait_idle().await;

        Ok(())
    }
}
