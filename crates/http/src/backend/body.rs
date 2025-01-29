use std::pin::Pin;

use bytes::Bytes;

use crate::backend::h3::body::QuicIncomingBody;

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
    type Error = crate::backend::error::Error;

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
