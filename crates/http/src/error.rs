use std::fmt::Debug;

use crate::service::{HttpService, HttpServiceFactory};

/// An error that can occur when creating or running an HTTP server.
#[derive(Debug, thiserror::Error)]
pub enum Error<F>
where
    F: HttpServiceFactory,
    F::Error: std::error::Error + Debug,
    <F::Service as HttpService>::Error: std::error::Error + Debug,
    <<F::Service as HttpService>::ResBody as http_body::Body>::Error: std::error::Error + Debug,
{
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    #[cfg(all(feature = "http3", feature = "tls-rustls"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "http3", feature = "tls-rustls"))))]
    NoInitialCipherSuite(#[from] h3_quinn::quinn::crypto::rustls::NoInitialCipherSuite),
    #[error("h3 error: {0}")]
    #[cfg(feature = "http3")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http3")))]
    H3(#[from] h3::Error),
    #[error("quinn connection error: {0}")]
    #[cfg(feature = "http3")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http3")))]
    QuinnConnection(#[from] h3_quinn::quinn::ConnectionError),
    #[error("make service error: {0}")]
    ServiceFactoryError(F::Error),
    #[error("service error: {0}")]
    ServiceError(<F::Service as HttpService>::Error),
    #[error("response body error: {0}")]
    ResBodyError(<<F::Service as HttpService>::ResBody as http_body::Body>::Error),
}
