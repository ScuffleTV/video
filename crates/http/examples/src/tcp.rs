use std::convert::Infallible;
use std::net::SocketAddr;

use bytes::Bytes;
use http::Response;
use scuffle_http::backend::tcp::{TcpServer, TcpServerConfig};
use scuffle_http::backend::HttpServer;
use scuffle_http::svc::function_service;

#[tokio::main]
async fn main() {
	tracing_subscriber::fmt().init();

	let tcp_server = TcpServer::new(
		TcpServerConfig::builder()
			.with_bind(SocketAddr::from(([0, 0, 0, 0], 8080)))
			.with_idle_timeout(std::time::Duration::from_secs(5))
			.build(),
	);

	tcp_server
		.start(
			function_service(|req| async move {
				let body = http_body_util::Full::new(Bytes::from(format!("hi tcp: {:?}", req)));
				Ok::<_, Infallible>(Response::builder().body(body).unwrap())
			}),
			1,
		)
		.await
		.unwrap();

	tracing::info!("Server started on tcp {:?}", tcp_server.local_addr().unwrap());
	tcp_server.wait().await.unwrap();
}
