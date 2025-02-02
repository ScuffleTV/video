use std::pin::Pin;

use bytes::Bytes;

use crate::backend::h3::body::QuicIncomingBody;

/// An error that can occur when reading the body of an incoming request.
#[derive(thiserror::Error, Debug)]
pub enum IncomingBodyError {
    #[error("hyper error: {0}")]
    Hyper(#[from] hyper::Error),
    #[error("quic error: {0}")]
    Quic(#[from] h3::Error),
}

/// The body of an incoming request.
///
/// This enum is used to abstract away the differences between the body types of HTTP/1, HTTP/2 and HTTP/3.
/// It implements the [`http_body::Body`] trait.
pub enum IncomingBody {
    Hyper(hyper::body::Incoming),
    Quic(QuicIncomingBody<h3_quinn::BidiStream<Bytes>>),
}

impl From<hyper::body::Incoming> for IncomingBody {
    fn from(body: hyper::body::Incoming) -> Self {
        IncomingBody::Hyper(body)
    }
}

impl From<QuicIncomingBody<h3_quinn::BidiStream<Bytes>>> for IncomingBody {
    fn from(body: QuicIncomingBody<h3_quinn::BidiStream<Bytes>>) -> Self {
        IncomingBody::Quic(body)
    }
}

impl http_body::Body for IncomingBody {
    type Data = Bytes;
    type Error = IncomingBodyError;

    fn is_end_stream(&self) -> bool {
        match self {
            IncomingBody::Hyper(body) => body.is_end_stream(),
            IncomingBody::Quic(body) => body.is_end_stream(),
        }
    }

    fn poll_frame(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        match self.get_mut() {
            IncomingBody::Hyper(body) => Pin::new(body).poll_frame(cx).map_err(Into::into),
            IncomingBody::Quic(body) => Pin::new(body).poll_frame(cx).map_err(Into::into),
        }
    }

    fn size_hint(&self) -> http_body::SizeHint {
        match self {
            IncomingBody::Hyper(body) => body.size_hint(),
            IncomingBody::Quic(body) => body.size_hint(),
        }
    }
}
