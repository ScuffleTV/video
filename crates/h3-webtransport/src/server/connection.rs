use std::collections::HashMap;

use bytes::Buf;
use futures_util::future::Either;
use h3::quic;
use h3::server::RequestStream;
use http::Request;
use tokio::sync::mpsc;

use super::{ConnectionDriver, Incoming};

// A WebTransport server that allows incoming requests to be upgraded to
// `WebTransportSessions`
//
// The [`WebTransportServer`] struct manages a connection from the side of the
// HTTP/3 server
//
// Create a new Instance with [`WebTransportServer::new()`].
// Accept incoming requests with [`WebTransportServer::accept()`].
// And shutdown a connection with [`WebTransportServer::shutdown()`].
pub struct Connection<C, B>
where
    C: quic::Connection<B>,
    B: Buf,
{
    pub(crate) incoming: Incoming<C, B>,
    pub(crate) driver: ConnectionDriver<C, B>,
}

impl<C, B, E, E2> Connection<C, B>
where
    C: quic::Connection<B> + quic::SendDatagramExt<B, Error = E> + quic::RecvDatagramExt<Buf = B, Error = E2> + 'static,
    B: Buf + 'static + Send + Sync,
    C::AcceptError: Send + Sync,
    C::BidiStream: Send + Sync,
    C::RecvStream: Send + Sync,
    C::OpenStreams: Send + Sync,
    E: Into<h3::Error>,
    E2: Into<h3::Error>,
{
    /// Create a new `WebTransportServer`
    pub fn new(inner: h3::server::Connection<C, B>) -> Self {
        let (request_sender, request_recv) = mpsc::channel(128);
        let (webtransport_request_tx, webtransport_request_rx) = mpsc::channel(128);
        let (session_close_tx, session_close_rx) = mpsc::unbounded_channel();

        Self {
            driver: ConnectionDriver {
                webtransport_session_map: HashMap::new(),
                request_sender,
                webtransport_request_rx,
                webtransport_request_tx,
                session_close_rx,
                session_close_tx,
                inner,
            },
            incoming: Incoming { recv: request_recv },
        }
    }

    /// Take the request acceptor
    pub fn split(self) -> (Incoming<C, B>, ConnectionDriver<C, B>) {
        (self.incoming, self.driver)
    }

    /// Get a mutable reference to the driver
    pub fn driver(&mut self) -> &mut ConnectionDriver<C, B> {
        &mut self.driver
    }

    /// Accepts an incoming request
    /// Internally this method will drive the server until an incoming request
    /// is available And returns the request and a request stream.
    pub async fn accept(&mut self) -> Result<Option<(Request<()>, RequestStream<C::BidiStream, B>)>, h3::Error> {
        match futures_util::future::select(std::pin::pin!(self.incoming.accept()), std::pin::pin!(self.driver.drive())).await
        {
            Either::Left((accept, _)) => Ok(accept),
            Either::Right((drive, _)) => drive.map(|_| None),
        }
    }
}

impl<C, B> Connection<C, B>
where
    C: quic::Connection<B>,
    B: Buf,
{
    /// Closes the connection with a code and a reason.
    pub fn close(&mut self, code: h3::error::Code, reason: &str) -> h3::Error {
        self.driver.close(code, reason)
    }
}
