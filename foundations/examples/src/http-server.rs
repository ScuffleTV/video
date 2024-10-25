use std::sync::Arc;

use axum::body::Body;
use axum::extract::Request;
use axum::response::{IntoResponse, Response};
use hyper::{StatusCode, Uri};
use quinn::crypto::rustls::QuicServerConfig;
use scuffle_foundations::bootstrap::{bootstrap, Bootstrap, RuntimeSettings};
use scuffle_foundations::http::server::stream::{IncomingConnection, MakeService, ServiceHandler, SocketKind};
use scuffle_foundations::http::server::Server;
use scuffle_foundations::settings::auto_settings;
use scuffle_foundations::settings::cli::{clap, Matches};
use scuffle_foundations::telemetry::settings::TelemetrySettings;
use tokio::signal::unix::SignalKind;

#[auto_settings]
pub struct HttpServerSettings {
	tls_cert: Option<String>,
	tls_key: Option<String>,

	/// Telementry Settings
	telementry: TelemetrySettings,
	/// Runtime Settings
	runtime: RuntimeSettings,
}

impl Bootstrap for HttpServerSettings {
	type Settings = Self;

	fn runtime_mode(&self) -> RuntimeSettings {
		self.runtime.clone()
	}

	fn telemetry_config(&self) -> Option<TelemetrySettings> {
		Some(self.telementry.clone())
	}

	fn additional_args() -> Vec<clap::Arg> {
		vec![
			clap::Arg::new("tls-cert").long("tls-cert").value_name("FILE"),
			clap::Arg::new("tls-key").long("tls-key").value_name("FILE"),
		]
	}
}

#[bootstrap]
async fn main(settings: Matches<HttpServerSettings>) {
	let tls_cert = settings
		.args
		.get_one::<String>("tls-cert")
		.or(settings.settings.tls_cert.as_ref());
	let tls_key = settings
		.args
		.get_one::<String>("tls-key")
		.or(settings.settings.tls_key.as_ref());

	let Some((tls_cert, tls_key)) = tls_cert.zip(tls_key) else {
		panic!("TLS certificate and key are required");
	};

	let cert = std::fs::File::open(tls_cert).expect("failed to open certificate file");
	let key = std::fs::File::open(tls_key).expect("failed to open key file");

	// Test TLS
	let certs = rustls_pemfile::certs(&mut std::io::BufReader::new(cert))
		.collect::<Result<Vec<_>, _>>()
		.unwrap();

	let key = rustls_pemfile::pkcs8_private_keys(&mut std::io::BufReader::new(key))
		.next()
		.unwrap()
		.unwrap();

	let mut tls_config = rustls::ServerConfig::builder_with_protocol_versions(&[&rustls::version::TLS13])
		.with_no_client_auth()
		.with_single_cert(certs, key.into())
		.unwrap();

	tls_config.max_early_data_size = u32::MAX;
	tls_config.alpn_protocols = vec![b"h3".to_vec()];

	let server_config = quinn::ServerConfig::with_crypto(Arc::new(QuicServerConfig::try_from(tls_config.clone()).unwrap()));

	#[derive(Debug, Clone)]
	struct ServiceFactory;

	#[derive(Debug, Clone)]
	struct NormalService;

	#[derive(Debug, Clone)]
	struct NoTlsService;

	impl ServiceHandler for NormalService {
		fn on_request(&self, _: Request) -> impl std::future::Future<Output = impl IntoResponse> + Send {
			std::future::ready(
				Response::builder()
					.status(StatusCode::OK)
					.header("Alt-Svc", "h3=\":18080\"; ma=2592000")
					.body(axum::body::Body::from("Hello, World!"))
					.unwrap(),
			)
		}
	}

	impl ServiceHandler for NoTlsService {
		async fn on_request(&self, request: Request) -> impl IntoResponse {
			let uri = request.uri();

			let mut location = Uri::builder().scheme("https");

			if let Some(host) = request.headers().get("host") {
				let host = host.to_str().unwrap();
				// Change the port to 18080
				let host = host.split(':').next().unwrap();
				let host = format!("{host}:18080");
				location = location.authority(host);
			}

			if let Some(path_and_query) = uri.path_and_query().cloned() {
				location = location.path_and_query(path_and_query);
			}

			let location = location.build().unwrap();

			Response::builder()
				.status(StatusCode::TEMPORARY_REDIRECT)
				.header("location", location.to_string())
				.body(Body::empty())
				.unwrap()
		}
	}

	#[derive(Debug, Clone)]
	enum AnyService {
		Normal(NormalService),
		NoTls(NoTlsService),
	}

	impl ServiceHandler for AnyService {
		async fn on_request(&self, request: Request) -> impl IntoResponse {
			match self {
				AnyService::Normal(service) => service.on_request(request).await.into_response(),
				AnyService::NoTls(service) => service.on_request(request).await.into_response(),
			}
		}
	}

	impl MakeService for ServiceFactory {
		fn make_service(
			&self,
			incoming: &impl IncomingConnection,
		) -> impl std::future::Future<Output = Option<impl ServiceHandler>> + Send {
			if incoming.socket_kind() == SocketKind::Tcp {
				std::future::ready(Some(AnyService::NoTls(NoTlsService)))
			} else {
				std::future::ready(Some(AnyService::Normal(NormalService)))
			}
		}
	}

	let mut builder = h3::server::builder();

	builder
		.send_grease(true)
		.enable_connect(true)
		.enable_datagram(true)
		.enable_webtransport(true)
		.max_webtransport_sessions(1);

	let mut server = Server::builder()
		.bind(([0, 0, 0, 0], 18080).into())
		.with_tls(tls_config)
		.with_insecure(([0, 0, 0, 0], 18081).into())
		.with_http3(builder, server_config)
		.build(ServiceFactory)
		.unwrap();

	server.start().await.unwrap();

	tracing::info!("server started");

	scuffle_foundations::signal::SignalHandler::new()
		.with_signal(SignalKind::interrupt())
		.with_signal(SignalKind::terminate())
		.recv()
		.await;

	tracing::info!("shutting down server");

	scuffle_foundations::context::Handler::global().shutdown().await;

	server.wait().await.unwrap();

	tracing::info!("server stopped");
}
