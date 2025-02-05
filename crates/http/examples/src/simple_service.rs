use std::{fs, io};

use rustls::pki_types::{CertificateDer, PrivateKeyDer};

#[tokio::main]
async fn main() {
    let service = scuffle_http::service::fn_http_service(|req| async move {
        scuffle_http::http::Response::builder()
            .status(scuffle_http::http::StatusCode::OK)
            .header(scuffle_http::http::header::CONTENT_TYPE, "text/plain")
            .body(format!("Hello, world!\nURL: {:?}", req.uri()))
    });
    let service_factory = scuffle_http::service::service_clone_factory(service);

    scuffle_http::HttpServer::builder()
        .with_rustls(get_tls_config().expect("failed to load tls config"))
        .with_service_factory(service_factory)
        .bind("[::]:443".parse().unwrap())
        .enable_http3()
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
