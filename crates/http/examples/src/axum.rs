use std::net::SocketAddr;
use std::{fs, io};

use axum::body::Body;
use axum::http::Request;
use axum::response::Response;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tracing::level_filters::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{fmt, EnvFilter};

async fn hello_world(req: Request<axum::body::Body>) -> axum::response::Response<String> {
    tracing::info!("received request: {} {}", req.method(), req.uri());

    let mut resp = axum::response::Response::new("Hello, World!\n".to_string());

    // TODO: this has to be part of the library somehow
    resp.headers_mut()
        .insert("Alt-Svc", "h3=\":443\"; ma=3600, h2=\":443\"; ma=3600".parse().unwrap());

    resp
}

async fn ws(ws: axum::extract::ws::WebSocketUpgrade) -> Response<Body> {
    ws.on_upgrade(|mut socket| async move {
        while let Some(msg) = socket.recv().await {
            let msg = msg.unwrap();
            socket.send(msg).await.unwrap();
        }
    })
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

    let make_service = axum::Router::<()>::new()
        .route("/", axum::routing::get(hello_world))
        .route("/ws", axum::routing::get(ws))
        .into_make_service_with_connect_info::<SocketAddr>();

    scuffle_http::HttpServer::builder()
        .with_rustls(get_tls_config().expect("failed to load tls config"))
        .with_service(make_service)
        .bind("[::]:443".parse().unwrap())
        .build()
        .expect("failed to build server")
        .run()
        .await
        .expect("server failed");
}

pub fn get_tls_config() -> io::Result<rustls::ServerConfig> {
    rustls::crypto::aws_lc_rs::default_provider().install_default().unwrap();

    let certs = load_certs("local/fullchain.pem")?;
    let key = load_private_key("local/privkey.pem")?;

    let server_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .unwrap();

    Ok(server_config)
}

// Load public certificate from file.
fn load_certs(filename: &str) -> io::Result<Vec<CertificateDer<'static>>> {
    // Open certificate file.
    let certfile = fs::File::open(filename)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("failed to open {}: {}", filename, e)))?;
    let mut reader = io::BufReader::new(certfile);

    // Load and return certificate.
    rustls_pemfile::certs(&mut reader).collect()
}

// Load private key from file.
fn load_private_key(filename: &str) -> io::Result<PrivateKeyDer<'static>> {
    // Open keyfile.
    let keyfile = fs::File::open(filename)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, format!("failed to open {}: {}", filename, e)))?;
    let mut reader = io::BufReader::new(keyfile);

    // Load and return a single private key.
    rustls_pemfile::private_key(&mut reader)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, format!("no private key found in {}", filename)))
}
