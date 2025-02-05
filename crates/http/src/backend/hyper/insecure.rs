use std::fmt::{Debug, Display};
use std::net::SocketAddr;

use crate::error::Error;
use crate::service::{HttpService, HttpServiceFactory};

#[derive(Debug, Clone)]
pub struct InsecureBackend {
    pub bind: SocketAddr,
    #[cfg(feature = "http1")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http1")))]
    pub http1_enabled: bool,
    #[cfg(feature = "http2")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http2")))]
    pub http2_enabled: bool,
}

impl InsecureBackend {
    pub async fn run<S>(self, mut service_factory: S) -> Result<(), Error<S>>
    where
        S: HttpServiceFactory,
        S::Error: Debug + Display,
        S::Service: Clone + Send + 'static,
        <S::Service as HttpService>::Error: std::error::Error + Debug + Display + Send + Sync,
        <S::Service as HttpService>::ResBody: Send,
        <<S::Service as HttpService>::ResBody as http_body::Body>::Data: Send,
        <<S::Service as HttpService>::ResBody as http_body::Body>::Error: std::error::Error + Send + Sync,
    {
        #[cfg(feature = "tracing")]
        tracing::debug!("starting server");

        let listener = tokio::net::TcpListener::bind(self.bind).await?;

        loop {
            let _res: Result<_, Error<S>> = async {
                let (tcp_stream, addr) = listener.accept().await?;

                #[cfg(all(feature = "http1", not(feature = "http2")))]
                super::handle_connection(&mut service_factory, addr, tcp_stream, self.http1_enabled).await?;

                #[cfg(all(not(feature = "http1"), feature = "http2"))]
                super::handle_connection(&mut service_factory, addr, tcp_stream, self.http2_enabled).await?;

                #[cfg(all(feature = "http1", feature = "http2"))]
                super::handle_connection(&mut service_factory, addr, tcp_stream, self.http1_enabled, self.http2_enabled)
                    .await?;

                Ok(())
            }
            .await;

            #[cfg(feature = "tracing")]
            if let Err(e) = _res {
                tracing::warn!("error: {}", e);
            }
        }
    }
}
