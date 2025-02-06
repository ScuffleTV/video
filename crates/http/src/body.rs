use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::{Buf, Bytes};
use http_body::Frame;

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

pin_project_lite::pin_project! {
    /// A wrapper around an HTTP body that tracks the size of the data that is read from it.
    pub struct TrackedBody<B, T> {
        #[pin]
        body: B,
        tracker: T,
    }
}

impl<B, T> TrackedBody<B, T> {
    pub fn new(body: B, tracker: T) -> Self {
        Self { body, tracker }
    }
}

/// An error that can occur when tracking the body of an incoming request.
pub enum TrackedBodyError<B, T>
where
    B: http_body::Body,
    T: Tracker,
{
    Body(B::Error),
    Tracker(T::Error),
}

/// A trait for tracking the size of the data that is read from an HTTP body.
pub trait Tracker: Send + Sync + 'static {
    type Error;

    /// Called when data is read from the body.
    ///
    /// The `size` parameter is the size of the data that is remaining to be read from the body.
    fn on_data(&self, size: usize) -> Result<(), Self::Error> {
        let _ = size;
        Ok(())
    }
}

impl<B, T> http_body::Body for TrackedBody<B, T>
where
    B: http_body::Body,
    T: Tracker,
{
    type Data = B::Data;
    type Error = TrackedBodyError<B, T>;

    fn poll_frame(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.project();
        match this.body.poll_frame(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(frame) => {
                if let Some(Ok(frame)) = &frame {
                    if let Some(data) = frame.data_ref() {
                        if let Err(err) = this.tracker.on_data(data.remaining()) {
                            return Poll::Ready(Some(Err(TrackedBodyError::Tracker(err))));
                        }
                    }
                }

                Poll::Ready(frame.transpose().map_err(TrackedBodyError::Body).transpose())
            }
        }
    }

    fn is_end_stream(&self) -> bool {
        self.body.is_end_stream()
    }

    fn size_hint(&self) -> http_body::SizeHint {
        self.body.size_hint()
    }
}
