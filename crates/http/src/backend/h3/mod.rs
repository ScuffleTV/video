use std::fmt::{Debug, Display};
use std::net::SocketAddr;
use std::sync::Arc;

use body::QuicIncomingBody;
use scuffle_context::ContextFutExt;
use utils::copy_response_body;

use crate::error::Error;
use crate::service::{HttpService, HttpServiceFactory};

pub mod body;
mod utils;

#[derive(Debug, Clone)]
pub struct Http3Backend {
    pub ctx: scuffle_context::Context,
    pub bind: SocketAddr,
}

impl Http3Backend {
    pub async fn run<S>(self, service_factory: S, mut rustls_config: rustls::ServerConfig) -> Result<(), Error<S>>
    where
        S: HttpServiceFactory + Clone + Send + 'static,
        S::Error: Debug + Display,
        S::Service: Clone + Send + 'static,
        <S::Service as HttpService>::Error: std::error::Error + Debug + Display + Send + Sync,
        <S::Service as HttpService>::ResBody: Send,
        <<S::Service as HttpService>::ResBody as http_body::Body>::Data: Send,
        <<S::Service as HttpService>::ResBody as http_body::Body>::Error: std::error::Error + Send + Sync,
    {
        #[cfg(feature = "tracing")]
        tracing::debug!("starting server");

        // not quite sure why this is necessary but it is
        rustls_config.max_early_data_size = u32::MAX;
        let crypto = h3_quinn::quinn::crypto::rustls::QuicServerConfig::try_from(rustls_config)?;
        let server_config = h3_quinn::quinn::ServerConfig::with_crypto(Arc::new(crypto));

        let endpoint = h3_quinn::quinn::Endpoint::server(server_config, self.bind)?;

        // handle incoming connections and requests
        while let Some(new_conn) = endpoint.accept().await {
            let mut service_factory = service_factory.clone();

            let ctx = self.ctx.clone();

            tokio::spawn(
                async move {
                    let _res: Result<_, Error<S>> = async move {
                        let conn = new_conn.await?;
                        let addr = conn.remote_address();

                        let mut h3_conn = h3::server::Connection::new(h3_quinn::Connection::new(conn)).await?;

                        loop {
                            match h3_conn.accept().await {
                                Ok(Some((req, stream))) => {
                                    let (mut send, recv) = stream.split();

                                    let size_hint = req
                                        .headers()
                                        .get(http::header::CONTENT_LENGTH)
                                        .and_then(|len| len.to_str().ok().and_then(|x| x.parse().ok()));
                                    let body = QuicIncomingBody::new(recv, size_hint);
                                    let req = req.map(|_| crate::body::IncomingBody::from(body));

                                    // make a new service
                                    let mut http_service = service_factory
                                        .new_service(addr)
                                        .await
                                        .map_err(|e| Error::ServiceFactoryError(e))?;

                                    tokio::spawn(
                                        async move {
                                            let _res: Result<_, Error<S>> = async move {
                                                let resp =
                                                    http_service.call(req).await.map_err(|e| Error::ServiceError(e))?;
                                                let (parts, body) = resp.into_parts();

                                                send.send_response(http::Response::from_parts(parts, ())).await?;

                                                copy_response_body(send, body).await?;

                                                Ok(())
                                            }
                                            .await;

                                            #[cfg(feature = "tracing")]
                                            if let Err(err) = _res {
                                                tracing::warn!("error: {}", err);
                                            }
                                        }
                                        .with_context(ctx.clone()),
                                    );
                                }
                                // indicating no more streams to be received
                                Ok(None) => {
                                    break;
                                }
                                Err(err) => match err.get_error_level() {
                                    h3::error::ErrorLevel::ConnectionError => return Err(err.into()),
                                    h3::error::ErrorLevel::StreamError => {
                                        #[cfg(feature = "tracing")]
                                        tracing::warn!("error on accept: {}", err);
                                        continue;
                                    }
                                },
                            }
                        }

                        Ok(())
                    }
                    .await;

                    #[cfg(feature = "tracing")]
                    if let Err(err) = _res {
                        tracing::warn!("error: {}", err);
                    }
                }
                .with_context(self.ctx.clone()),
            );
        }

        // shut down gracefully
        // wait for connections to be closed before exiting
        endpoint.wait_idle().await;

        Ok(())
    }
}
