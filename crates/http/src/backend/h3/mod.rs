use std::fmt::{Debug, Display};
use std::future::poll_fn;
use std::net::SocketAddr;
use std::sync::Arc;

use body::QuicIncomingBody;
use h3_quinn::BidiStream;
use utils::copy_response_body;

use crate::error::Error;

pub mod body;
mod utils;

#[derive(Debug, Clone)]
pub struct Http3Backend {
    pub bind: SocketAddr,
}

impl Http3Backend {
    pub async fn run<M, D>(self, make_service: M, mut rustls_config: rustls::ServerConfig) -> Result<(), Error<M>>
    where
        M: tower::MakeService<SocketAddr, crate::backend::IncomingRequest, Response = hyper::Response<D>>
            + Clone
            + Send
            + 'static,
        M::Error: std::error::Error + Display + Send + Sync + 'static,
        M::Service: Send + Clone + 'static,
        <M::Service as tower::Service<crate::backend::IncomingRequest>>::Future: Send,
        M::MakeError: Debug + Display,
        M::Future: Send,
        D: http_body::Body + Send + 'static,
        D::Error: Into<Box<dyn std::error::Error + Send + Sync>> + Send,
        D::Data: Send,
    {
        tracing::debug!("starting server");

        // not quite sure why this is necessary but it is
        rustls_config.max_early_data_size = u32::MAX;
        let crypto = h3_quinn::quinn::crypto::rustls::QuicServerConfig::try_from(rustls_config)?;
        let server_config = h3_quinn::quinn::ServerConfig::with_crypto(Arc::new(crypto));

        let endpoint = h3_quinn::quinn::Endpoint::server(server_config, self.bind)?;

        // handle incoming connections and requests
        while let Some(new_conn) = endpoint.accept().await {
            let mut make_service = make_service.clone();

            tokio::spawn(async move {
                let res: Result<_, Error<M>> = async move {
                    let conn = new_conn.await?;
                    let addr = conn.remote_address();

                    let mut h3_conn = h3::server::Connection::new(h3_quinn::Connection::new(conn)).await?;

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
                                    .map_err(|e| Error::MakeServiceError(e))?;
                                let mut tower_service = tower::MakeService::make_service(&mut make_service, addr)
                                    .await
                                    .map_err(|e| Error::MakeServiceError(e))?;

                                tokio::spawn(async move {
                                    let res: Result<_, Error<M>> = async move {
                                        let resp = tower::Service::call(&mut tower_service, req)
                                            .await
                                            .map_err(|e| Error::ServiceError(e))?;
                                        let (parts, body) = resp.into_parts();

                                        send.send_response(hyper::Response::from_parts(parts, ())).await?;

                                        copy_response_body(send, body).await;

                                        Ok(())
                                    }
                                    .await;

                                    if let Err(err) = res {
                                        tracing::warn!("error: {}", err);
                                    }
                                });
                            }
                            // indicating no more streams to be received
                            Ok(None) => {
                                break;
                            }
                            Err(err) => match err.get_error_level() {
                                h3::error::ErrorLevel::ConnectionError => return Err(err.into()),
                                h3::error::ErrorLevel::StreamError => {
                                    tracing::warn!("error on accept: {}", err);
                                    continue;
                                }
                            },
                        }
                    }

                    Ok(())
                }
                .await;

                if let Err(err) = res {
                    tracing::warn!("error: {}", err);
                }
            });
        }

        // shut down gracefully
        // wait for connections to be closed before exiting
        endpoint.wait_idle().await;

        Ok(())
    }
}
