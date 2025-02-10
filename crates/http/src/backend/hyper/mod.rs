use std::fmt::Debug;
use std::net::SocketAddr;
use std::sync::Arc;

use scuffle_context::ContextFutExt;
#[cfg(feature = "tracing")]
use tracing::Instrument;

use crate::error::Error;
use crate::service::{HttpService, HttpServiceFactory};

mod handler;
mod stream;

/// A backend that handles incoming HTTP connections using a hyper backend.
///
/// This is used internally by the [`HttpServer`](crate::server::HttpServer) but can be used directly if preferred.
///
/// Call [`run`](HyperBackend::run) to start the server.
#[derive(Debug, Clone, bon::Builder)]
pub struct HyperBackend<F> {
    #[builder(default)]
    ctx: scuffle_context::Context,
    #[builder(default = 1)]
    worker_tasks: usize,
    service_factory: F,
    bind: SocketAddr,
    #[cfg(feature = "tls-rustls")]
    #[cfg_attr(docsrs, doc(cfg(feature = "tls-rustls")))]
    rustls_config: Option<rustls::ServerConfig>,
    #[cfg(feature = "http1")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http1")))]
    #[builder(default = true)]
    http1_enabled: bool,
    #[cfg(feature = "http2")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http2")))]
    #[builder(default = true)]
    http2_enabled: bool,
}

impl<F> HyperBackend<F>
where
    F: HttpServiceFactory + Clone + Send + 'static,
    F::Error: std::error::Error + Debug,
    F::Service: Clone + Send + 'static,
    <F::Service as HttpService>::Error: std::error::Error + Debug + Send + Sync,
    <F::Service as HttpService>::ResBody: Send,
    <<F::Service as HttpService>::ResBody as http_body::Body>::Data: Send,
    <<F::Service as HttpService>::ResBody as http_body::Body>::Error: std::error::Error + Send + Sync,
{
    /// Run the HTTP server
    ///
    /// This function will bind to the address specified in `bind`, listen for incoming connections and handle requests.
    #[cfg_attr(feature = "tracing", tracing::instrument(skip_all, fields(bind = %self.bind)))]
    pub async fn run(mut self) -> Result<(), Error<F>> {
        #[cfg(feature = "tracing")]
        tracing::debug!("starting server");

        // reset to 0 because everything explodes if it's not
        // https://github.com/hyperium/hyper/issues/3841
        #[cfg(feature = "tls-rustls")]
        if let Some(rustls_config) = self.rustls_config.as_mut() {
            rustls_config.max_early_data_size = 0;
        }

        // We have to create an std listener first because the tokio listener isn't clonable
        let std_listener = std::net::TcpListener::bind(self.bind)?;
        // Set nonblocking so we can use it in the async runtime
        // This should be the default when converting to a tokio listener
        std_listener.set_nonblocking(true)?;

        #[cfg(feature = "tls-rustls")]
        let tls_acceptor = self.rustls_config.map(|c| tokio_rustls::TlsAcceptor::from(Arc::new(c)));

        let tasks = (0..self.worker_tasks).map(|n| {
            let service_factory = self.service_factory.clone();
            let ctx = self.ctx.clone();
            let std_listener = std_listener.try_clone().expect("failed to clone listener");
            let listener = tokio::net::TcpListener::from_std(std_listener).expect("failed to create tokio listener");
            let tls_acceptor = tls_acceptor.clone();

            let worker_fut = async move {
                loop {
                    #[cfg(feature = "tracing")]
                    tracing::trace!("waiting for connections");

                    let (mut stream, addr) = match listener.accept().with_context(ctx.clone()).await {
                        Some(Ok((tcp_stream, addr))) => (stream::Stream::Tcp(tcp_stream), addr),
                        #[cfg(feature = "tracing")]
                        Some(Err(e)) => {
                            tracing::warn!(err = %e, "failed to accept tcp connection");
                            continue;
                        }
                        #[cfg(not(feature = "tracing"))]
                        Some(Err(_)) => continue,
                        None => {
                            #[cfg(feature = "tracing")]
                            tracing::trace!("context done, stopping listener");
                            break;
                        }
                    };

                    #[cfg(feature = "tracing")]
                    tracing::trace!(addr = %addr, "accepted tcp connection");

                    let ctx = ctx.clone();
                    let tls_acceptor = tls_acceptor.clone();
                    let mut service_factory = service_factory.clone();

                    let connection_fut = async move {
                        // Perform the TLS handshake if the acceptor is set
                        if let Some(tls_acceptor) = tls_acceptor {
                            #[cfg(feature = "tracing")]
                            tracing::trace!("accepting tls connection");

                            stream = match stream.try_accept_tls(&tls_acceptor).with_context(&ctx).await {
                                Some(Ok(stream)) => stream,
                                #[cfg(feature = "tracing")]
                                Some(Err(err)) => {
                                    tracing::warn!(err = %err, "failed to accept tls connection");
                                    return;
                                }
                                #[cfg(not(feature = "tracing"))]
                                Some(Err(_)) => return,
                                None => {
                                    #[cfg(feature = "tracing")]
                                    tracing::trace!("context done, stopping tls acceptor");
                                    return;
                                }
                            };

                            #[cfg(feature = "tracing")]
                            tracing::trace!("accepted tls connection");
                        }

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

                        #[cfg(feature = "tracing")]
                        tracing::trace!("handling connection");

                        #[cfg(all(feature = "http1", not(feature = "http2")))]
                        let _res =
                            handler::handle_connection::<F, _, _>(ctx, http_service, stream, self.http1_enabled).await;

                        #[cfg(all(not(feature = "http1"), feature = "http2"))]
                        let _res =
                            handler::handle_connection::<F, _, _>(ctx, http_service, stream, self.http2_enabled).await;

                        #[cfg(all(feature = "http1", feature = "http2"))]
                        let _res = handler::handle_connection::<F, _, _>(
                            ctx,
                            http_service,
                            stream,
                            self.http1_enabled,
                            self.http2_enabled,
                        )
                        .await;

                        #[cfg(feature = "tracing")]
                        if let Err(e) = _res {
                            tracing::warn!(err = %e, "error handling connection");
                        }

                        #[cfg(feature = "tracing")]
                        tracing::trace!("connection closed");
                    };

                    #[cfg(feature = "tracing")]
                    let connection_fut = connection_fut.instrument(tracing::trace_span!("connection", addr = %addr));

                    tokio::spawn(connection_fut);
                }

                #[cfg(feature = "tracing")]
                tracing::trace!("listener closed");
            };

            #[cfg(feature = "tracing")]
            let worker_fut = worker_fut.instrument(tracing::trace_span!("worker", n = n));

            worker_fut
        });

        futures::future::join_all(tasks).await;

        #[cfg(feature = "tracing")]
        tracing::debug!("all workers finished");

        Ok(())
    }
}
