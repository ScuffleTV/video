use std::future::Future;

use bytes::Bytes;
use scuffle_h3_webtransport::session::WebTransportSession;

// TODO: make this generic over any quic connection
/// Upgrades a request to a webtransport session
pub async fn upgrade_webtransport<B, F, Fut>(request: &mut http::Request<B>, on_upgrade: F) -> Option<http::Response<()>>
where
	F: FnOnce(WebTransportSession<h3_quinn::Connection, Bytes>) -> Fut + Send + Sync + 'static,
	Fut: Future<Output = ()> + Send + Sync + 'static,
{
	WebTransportSession::begin(request, on_upgrade)
}
