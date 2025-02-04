use std::fmt::{Debug, Display};

use crate::service::{HttpService, HttpServiceFactory};

/// An error that can occur when creating or running an HTTP server.
#[derive(Debug, thiserror::Error)]
pub enum Error<S>
where
    S: HttpServiceFactory,
    S::Error: Debug + Display,
    <S::Service as HttpService>::Error: Debug + Display,
{
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    #[cfg(all(feature = "http3", feature = "tls-rustls"))]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "http3", feature = "tls-rustls"))))]
    NoInitialCipherSuite(#[from] quinn::crypto::rustls::NoInitialCipherSuite),
    #[error("h3 error: {0}")]
    #[cfg(feature = "http3")]
    #[cfg_attr(docsrs, doc(feature = "http3"))]
    H3(#[from] h3::Error),
    #[error("quinn connection error: {0}")]
    #[cfg(feature = "http3")]
    #[cfg_attr(docsrs, doc(feature = "http3"))]
    QuinnConnection(#[from] h3_quinn::quinn::ConnectionError),
    #[error("make service error: {0}")]
    ServiceFactoryError(S::Error),
    #[error("service error: {0}")]
    ServiceError(<S::Service as HttpService>::Error),
}
