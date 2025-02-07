use std::fmt::{Debug, Display};
use std::net::SocketAddr;

use scuffle_context::ContextFutExt;

use crate::error::Error;
use crate::service::{HttpService, HttpServiceFactory};

/// A backend that handles incoming HTTP (no TLS) connections.
///
/// This is used internally by the [`HttpServer`](crate::server::HttpServer) but can be used directly if preferred.
///
/// Call [`run`](InsecureBackend::run) to start the server.
#[derive(Debug, Clone)]
pub struct InsecureBackend {
    pub ctx: scuffle_context::Context,
    pub bind: SocketAddr,
    #[cfg(feature = "http1")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http1")))]
    pub http1_enabled: bool,
    #[cfg(feature = "http2")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http2")))]
    pub http2_enabled: bool,
}

impl InsecureBackend {
    /// Run the HTTP server
    ///
    /// This function will bind to the address specified in `bind`, listen for incoming connections and handle requests.
    #[tracing::instrument(skip_all, fields(bind = %self.bind))]
    pub async fn run<F>(self, service_factory: F) -> Result<(), Error<F>>
    where
        F: HttpServiceFactory + Clone + Send + 'static,
        F::Error: Debug + Display,
        F::Service: Clone + Send + 'static,
        <F::Service as HttpService>::Error: std::error::Error + Debug + Display + Send + Sync,
        <F::Service as HttpService>::ResBody: Send,
        <<F::Service as HttpService>::ResBody as http_body::Body>::Data: Send,
        <<F::Service as HttpService>::ResBody as http_body::Body>::Error: std::error::Error + Send + Sync,
    {
        #[cfg(feature = "tracing")]
        tracing::debug!("starting server");

        let listener = tokio::net::TcpListener::bind(self.bind).await?;

        loop {
            let (tcp_stream, addr) = match listener.accept().with_context(&self.ctx).await {
                Some(Ok(conn)) => conn,
                #[cfg(feature = "tracing")]
                Some(Err(e)) => {
                    tracing::warn!(err = %e, "failed to accept tcp connection");
                    continue;
                }
                #[cfg(not(feature = "tracing"))]
                Some(Err(_)) => continue,
                None => break,
            };

            let mut service_factory = service_factory.clone();
            tokio::spawn(
                async move {
                    // make a new service
                    let http_service = match service_factory.new_service(addr).await {
                        Ok(service) => service,
                        #[cfg(feature = "tracing")]
                        Err(e) => {
                            tracing::warn!(err = %e, "failed to create service");
                            return;
                        }
                        #[cfg(not(feature = "tracing"))]
                        Err(_) => return,
                    };

                    #[cfg(all(feature = "http1", not(feature = "http2")))]
                    let _res = super::handle_connection::<F, _, _>(http_service, tcp_stream, self.http1_enabled).await;

                    #[cfg(all(not(feature = "http1"), feature = "http2"))]
                    let _res = super::handle_connection::<F, _, _>(http_service, tcp_stream, self.http2_enabled).await;

                    #[cfg(all(feature = "http1", feature = "http2"))]
                    let _res = super::handle_connection::<F, _, _>(
                        http_service,
                        tcp_stream,
                        self.http1_enabled,
                        self.http2_enabled,
                    )
                    .await;

                    #[cfg(feature = "tracing")]
                    if let Err(e) = _res {
                        tracing::warn!("error: {}", e);
                    }
                }
                .with_context(self.ctx.clone()),
            );
        }

        Ok(())
    }
}
