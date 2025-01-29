#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("hyper error: {0}")]
    Hyper(#[from] hyper::Error),
    #[error("quic error: {0}")]
    Quic(#[from] h3::Error),
}
