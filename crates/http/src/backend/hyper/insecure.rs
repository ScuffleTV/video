use std::fmt::{Debug, Display};
use std::net::SocketAddr;

use crate::error::Error;
use crate::service::{HttpService, HttpServiceFactory};

#[derive(Debug, Clone)]
pub struct InsecureBackend {
    pub bind: SocketAddr,
    pub http1_enabled: bool,
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
        tracing::debug!("starting server");

        let listener = tokio::net::TcpListener::bind(self.bind).await?;

        loop {
            let res: Result<_, Error<S>> = async {
                let (tcp_stream, addr) = listener.accept().await?;
                super::handle_connection(&mut service_factory, addr, tcp_stream, self.http1_enabled, self.http2_enabled)
                    .await?;

                Ok(())
            }
            .await;

            if let Err(e) = res {
                tracing::warn!("error: {}", e);
            }
        }
    }
}
