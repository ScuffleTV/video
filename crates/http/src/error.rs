use std::{fmt::{Debug, Display}, net::SocketAddr};

use crate::backend::IncomingRequest;

#[derive(Debug, thiserror::Error)]
pub enum Error<M>
where
    M: tower::MakeService<SocketAddr, IncomingRequest>,
    M::MakeError: Debug + Display,
    M::Error: Debug + Display,
{
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    NoInitialCipherSuite(#[from] quinn::crypto::rustls::NoInitialCipherSuite),
    #[error("h3 error: {0}")]
    H3(#[from] h3::Error),
    #[error("quinn connection error: {0}")]
    QuinnConnection(#[from] h3_quinn::quinn::ConnectionError),
    #[error("make service error: {0}")]
    MakeServiceError(M::MakeError),
    #[error("service error: {0}")]
    ServiceError(M::Error),
}
