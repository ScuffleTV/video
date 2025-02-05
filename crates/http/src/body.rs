use bytes::Bytes;

/// An error that can occur when reading the body of an incoming request.
#[derive(thiserror::Error, Debug)]
pub enum IncomingBodyError {
    #[error("hyper error: {0}")]
    #[cfg(any(feature = "http1", feature = "http2"))]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "http1", feature = "http2"))))]
    Hyper(#[from] hyper::Error),
    #[error("quic error: {0}")]
    #[cfg(feature = "http3")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http3")))]
    Quic(#[from] h3::Error),
}

/// The body of an incoming request.
///
/// This enum is used to abstract away the differences between the body types of HTTP/1, HTTP/2 and HTTP/3.
/// It implements the [`http_body::Body`] trait.
pub enum IncomingBody {
    #[cfg(any(feature = "http1", feature = "http2"))]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "http1", feature = "http2"))))]
    Hyper(hyper::body::Incoming),
    #[cfg(feature = "http3")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http3")))]
    Quic(crate::backend::h3::body::QuicIncomingBody<h3_quinn::BidiStream<Bytes>>),
}

#[cfg(any(feature = "http1", feature = "http2"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "http1", feature = "http2"))))]
impl From<hyper::body::Incoming> for IncomingBody {
    fn from(body: hyper::body::Incoming) -> Self {
        IncomingBody::Hyper(body)
    }
}

#[cfg(feature = "http3")]
#[cfg_attr(docsrs, doc(cfg(feature = "http3")))]
impl From<crate::backend::h3::body::QuicIncomingBody<h3_quinn::BidiStream<Bytes>>> for IncomingBody {
    fn from(body: crate::backend::h3::body::QuicIncomingBody<h3_quinn::BidiStream<Bytes>>) -> Self {
        IncomingBody::Quic(body)
    }
}

impl http_body::Body for IncomingBody {
    type Data = Bytes;
    type Error = IncomingBodyError;

    fn is_end_stream(&self) -> bool {
        #[allow(unreachable_patterns)]
        match self {
            #[cfg(any(feature = "http1", feature = "http2"))]
            IncomingBody::Hyper(body) => body.is_end_stream(),
            #[cfg(feature = "http3")]
            IncomingBody::Quic(body) => body.is_end_stream(),
            #[cfg(not(any(feature = "http1", feature = "http2", feature = "http3")))]
            _ => false,
        }
    }

    fn poll_frame(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
        #[allow(unreachable_patterns)]
        match self.get_mut() {
            #[cfg(any(feature = "http1", feature = "http2"))]
            IncomingBody::Hyper(body) => std::pin::Pin::new(body).poll_frame(_cx).map_err(Into::into),
            #[cfg(feature = "http3")]
            IncomingBody::Quic(body) => std::pin::Pin::new(body).poll_frame(_cx).map_err(Into::into),
            #[cfg(not(any(feature = "http1", feature = "http2", feature = "http3")))]
            _ => std::task::Poll::Ready(None),
        }
    }

    fn size_hint(&self) -> http_body::SizeHint {
        #[allow(unreachable_patterns)]
        match self {
            #[cfg(any(feature = "http1", feature = "http2"))]
            IncomingBody::Hyper(body) => body.size_hint(),
            #[cfg(feature = "http3")]
            IncomingBody::Quic(body) => body.size_hint(),
            #[cfg(not(any(feature = "http1", feature = "http2", feature = "http3")))]
            _ => http_body::SizeHint::default(),
        }
    }
}
