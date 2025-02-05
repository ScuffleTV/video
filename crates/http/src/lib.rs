//! An HTTP server with support for HTTP/1, HTTP/2 and HTTP/3.
//!
//! It abstracts away [`hyper`](https://crates.io/crates/hyper) and [`h3`](https://crates.io/crates/h3) to provide a rather simple interface for creating and running a server that can handle all three protocols.
//!
//! See the [examples](./examples) directory for usage examples.
//!
//! ## Why do we need this?
//!
//! This crate is designed to be a simple and easy to use HTTP server that supports HTTP/1, HTTP/2 and HTTP/3.
//!
//! Currently, there are simply no other crates that provide support for all three protocols with a unified API.
//! This crate aims to fill that gap.
//!
//! ## Feature Flags
//!
//! - `tower`: Enables support for [`tower`](https://crates.io/crates/tower) services. Enabled by default.
//! - `http1`: Enables support for HTTP/1. Enabled by default.
//! - `http2`: Enables support for HTTP/2. Enabled by default.
//! - `http3`: Enables support for HTTP/3. Disabled by default.
//! - `tracing`: Enables logging with [`tracing`](https://crates.io/crates/tracing). Disabled by default.
//! - `tls-rustls`: Enables support for TLS with [`rustls`](https://crates.io/crates/rustls). Disabled by default.
//!
//! ## Example
//!
//! The following example demonstrates how to create a simple HTTP server (without TLS) that responds with "Hello, world!" to all requests on port 3000.
//!
//! ```rust
//! # use scuffle_future_ext::FutureExt;
//! # tokio_test::block_on(async {
//! # let run = async {
//! let service = scuffle_http::service::fn_http_service(|req| async move {
//!     scuffle_http::http::Response::builder()
//!     .status(scuffle_http::http::StatusCode::OK)
//!     .header(scuffle_http::http::header::CONTENT_TYPE, "text/plain")
//!     .body("Hello, world!".to_string())
//! });
//! let service_factory = scuffle_http::service::service_clone_factory(service);
//!
//! scuffle_http::HttpServer::builder()
//!     .service_factory(service_factory)
//!     .bind("[::]:3000".parse().unwrap())
//!     .build()
//!     .run()
//!     .await
//!     .expect("server failed");
//! # };
//! # run.with_timeout(std::time::Duration::from_secs(1)).await.expect_err("test should have timed out");
//! # });
//! ```
//!
//! ## Status
//!
//! This crate is currently under development and is not yet stable.
//!
//! Unit tests are not yet fully implemented. Use at your own risk.
//!
//! ### Missing Features
//!
//! - HTTP/3 webtransport support
//! - Upgrading to websocket connections from HTTP/3 connections (this is usually done via HTTP/1.1 anyway)
//!
//! ## License
//!
//! This project is licensed under the [MIT](./LICENSE.MIT) or [Apache-2.0](./LICENSE.Apache-2.0) license.
//! You can choose between one of them if you use this work.
//!
//! `SPDX-License-Identifier: MIT OR Apache-2.0`
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(all(coverage_nightly, test), feature(coverage_attribute))]

#[cfg(any(feature = "http1", feature = "http2", feature = "http3"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "http1", feature = "http2", feature = "http3"))))]
pub mod backend;
pub mod body;
pub mod error;
mod server;
pub mod service;

pub use http;
// pub use server::builder::ServerBuilder;
pub use server::HttpServer;

pub type IncomingRequest = http::Request<body::IncomingBody>;

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use std::convert::Infallible;
    use std::fmt::{Debug, Display};
    use std::fs;
    use std::io::BufReader;
    use std::net::SocketAddr;
    use std::time::Duration;

    use scuffle_future_ext::FutureExt;

    use crate::server::HttpServerBuilder;
    use crate::service::{fn_http_service, service_clone_factory, HttpService, HttpServiceFactory};
    use crate::HttpServer;

    fn get_available_addr() -> std::io::Result<std::net::SocketAddr> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
        listener.local_addr()
    }

    const RESPONSE_TEXT: &str = "Hello, world!";

    async fn test_server<F, S>(builder: HttpServerBuilder<F, S>, versions: &[reqwest::Version])
    where
        F: HttpServiceFactory + Debug + Clone + Send + 'static,
        F::Error: Debug + Display,
        F::Service: Clone + Debug + Send + 'static,
        <F::Service as HttpService>::Error: std::error::Error + Debug + Display + Send + Sync,
        <F::Service as HttpService>::ResBody: Send,
        <<F::Service as HttpService>::ResBody as http_body::Body>::Data: Send,
        <<F::Service as HttpService>::ResBody as http_body::Body>::Error: std::error::Error + Send + Sync,
        S: crate::server::http_server_builder::State,
        S::ServiceFactory: crate::server::http_server_builder::IsSet,
        S::Bind: crate::server::http_server_builder::IsUnset,
        S::Ctx: crate::server::http_server_builder::IsUnset,
    {
        let addr = get_available_addr().expect("failed to get available address");
        let (ctx, handler) = scuffle_context::Context::new();

        let server = builder.bind(addr).ctx(ctx).build();

        let handle = tokio::spawn(async move {
            server.run().await.expect("server run failed");
        });

        // Wait for the server to start
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let url = format!("http://{}/", addr);

        for version in versions {
            let mut builder = reqwest::Client::builder().danger_accept_invalid_certs(true);

            if *version == reqwest::Version::HTTP_3 {
                builder = builder.http3_prior_knowledge();
            } else if *version == reqwest::Version::HTTP_2 {
                builder = builder.http2_prior_knowledge();
            } else {
                builder = builder.http1_only();
            }

            let client = builder.build().expect("failed to build client");

            let request = client
                .request(reqwest::Method::GET, &url)
                .version(*version)
                .body(RESPONSE_TEXT.to_string())
                .build()
                .expect("failed to build request");

            let resp = client
                .execute(request)
                .await
                .expect("failed to get response")
                .text()
                .await
                .expect("failed to get text");

            assert_eq!(resp, RESPONSE_TEXT);
        }

        handler.shutdown().await;
        handle.await.expect("task failed");
    }

    async fn test_tls_server<F, S>(builder: HttpServerBuilder<F, S>, versions: &[reqwest::Version])
    where
        F: HttpServiceFactory + Debug + Clone + Send + 'static,
        F::Error: Debug + Display,
        F::Service: Clone + Debug + Send + 'static,
        <F::Service as HttpService>::Error: std::error::Error + Debug + Display + Send + Sync,
        <F::Service as HttpService>::ResBody: Send,
        <<F::Service as HttpService>::ResBody as http_body::Body>::Data: Send,
        <<F::Service as HttpService>::ResBody as http_body::Body>::Error: std::error::Error + Send + Sync,
        S: crate::server::http_server_builder::State,
        S::ServiceFactory: crate::server::http_server_builder::IsSet,
        S::Bind: crate::server::http_server_builder::IsUnset,
        S::Ctx: crate::server::http_server_builder::IsUnset,
    {
        let addr = get_available_addr().expect("failed to get available address");
        let (ctx, handler) = scuffle_context::Context::new();

        let server = builder.bind(addr).ctx(ctx).build();

        let handle = tokio::spawn(async move {
            server.run().await.expect("server run failed");
        });

        // Wait for the server to start
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let url = format!("https://{}/", addr);

        for version in versions {
            let mut builder = reqwest::Client::builder().danger_accept_invalid_certs(true).https_only(true);

            if *version == reqwest::Version::HTTP_3 {
                builder = builder.http3_prior_knowledge();
            } else if *version == reqwest::Version::HTTP_2 {
                builder = builder.http2_prior_knowledge();
            } else {
                builder = builder.http1_only();
            }

            let client = builder.build().expect("failed to build client");

            let request = client
                .request(reqwest::Method::GET, &url)
                .version(*version)
                .body(RESPONSE_TEXT.to_string())
                .build()
                .expect("failed to build request");

            let resp = client
                .execute(request)
                .await
                .unwrap_or_else(|_| panic!("failed to get response version {:?}", version))
                .text()
                .await
                .expect("failed to get text");

            assert_eq!(resp, RESPONSE_TEXT);
        }

        handler.shutdown().await;
        handle.await.expect("task failed");
    }

    #[tokio::test]
    async fn http2_server() {
        let builder = HttpServer::builder()
            .service_factory(service_clone_factory(fn_http_service(|_| async {
                Ok::<_, Infallible>(http::Response::new(RESPONSE_TEXT.to_string()))
            })))
            .enable_http1(false);

        test_server(builder, &[reqwest::Version::HTTP_2]).await;
    }

    #[tokio::test]
    async fn http12_server() {
        let server = HttpServer::builder().service_factory(service_clone_factory(fn_http_service(|_| async {
            Ok::<_, Infallible>(http::Response::new(RESPONSE_TEXT.to_string()))
        })));

        test_server(server, &[reqwest::Version::HTTP_11, reqwest::Version::HTTP_2]).await;
    }

    fn rustls_config() -> rustls::ServerConfig {
        rustls::crypto::aws_lc_rs::default_provider()
            .install_default()
            .expect("failed to install aws lc provider");

        let certfile = fs::File::open("assets/cert.pem").expect("cert not found");
        let certs = rustls_pemfile::certs(&mut BufReader::new(certfile))
            .collect::<Result<Vec<_>, _>>()
            .expect("failed to load certs");
        let keyfile = fs::File::open("assets/key.pem").expect("key not found");
        let key = rustls_pemfile::private_key(&mut BufReader::new(keyfile))
            .expect("failed to load key")
            .expect("no key found");

        rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .expect("failed to build config")
    }

    #[tokio::test]
    async fn rustls_http1_server() {
        let builder = HttpServer::builder()
            .service_factory(service_clone_factory(fn_http_service(|_| async {
                Ok::<_, Infallible>(http::Response::new(RESPONSE_TEXT.to_string()))
            })))
            .rustls_config(rustls_config())
            .enable_http2(false);

        test_tls_server(builder, &[reqwest::Version::HTTP_11]).await;
    }

    #[tokio::test]
    async fn rustls_http3_server() {
        let builder = HttpServer::builder()
            .service_factory(service_clone_factory(fn_http_service(|_| async {
                Ok::<_, Infallible>(http::Response::new(RESPONSE_TEXT.to_string()))
            })))
            .rustls_config(rustls_config())
            .enable_http1(false)
            .enable_http2(false)
            .enable_http3(true);

        test_tls_server(builder, &[reqwest::Version::HTTP_3]).await;
    }

    #[tokio::test]
    async fn rustls_http12_server() {
        let builder = HttpServer::builder()
            .service_factory(service_clone_factory(fn_http_service(|_| async {
                Ok::<_, Infallible>(http::Response::new(RESPONSE_TEXT.to_string()))
            })))
            .rustls_config(rustls_config());

        test_tls_server(builder, &[reqwest::Version::HTTP_11, reqwest::Version::HTTP_2]).await;
    }

    #[tokio::test]
    async fn rustls_http123_server() {
        let builder = HttpServer::builder()
            .service_factory(service_clone_factory(fn_http_service(|_| async {
                Ok::<_, Infallible>(http::Response::new(RESPONSE_TEXT.to_string()))
            })))
            .rustls_config(rustls_config())
            .enable_http3(true);

        test_tls_server(
            builder,
            &[reqwest::Version::HTTP_11, reqwest::Version::HTTP_2, reqwest::Version::HTTP_3],
        )
        .await;
    }

    #[tokio::test]
    async fn no_backend() {
        let addr = get_available_addr().expect("failed to get available address");

        let builder = HttpServer::builder()
            .service_factory(service_clone_factory(fn_http_service(|_| async {
                Ok::<_, Infallible>(http::Response::new(RESPONSE_TEXT.to_string()))
            })))
            .bind(addr)
            .enable_http1(false)
            .enable_http2(false);

        builder
            .build()
            .run()
            .with_timeout(Duration::from_millis(100))
            .await
            .expect("server timed out")
            .expect("server failed");
    }

    #[tokio::test]
    async fn rustls_no_backend() {
        let addr = get_available_addr().expect("failed to get available address");

        let builder = HttpServer::builder()
            .service_factory(service_clone_factory(fn_http_service(|_| async {
                Ok::<_, Infallible>(http::Response::new(RESPONSE_TEXT.to_string()))
            })))
            .rustls_config(rustls_config())
            .bind(addr)
            .enable_http1(false)
            .enable_http2(false);

        builder
            .build()
            .run()
            .with_timeout(Duration::from_millis(100))
            .await
            .expect("server timed out")
            .expect("server failed");
    }

    #[tokio::test]
    async fn tower_make_service() {
        let builder = HttpServer::builder().tower_make_service_factory(tower::service_fn(|_| async {
            Ok::<_, Infallible>(tower::service_fn(|_| async move {
                Ok::<_, Infallible>(http::Response::new(RESPONSE_TEXT.to_string()))
            }))
        }));

        test_server(builder, &[reqwest::Version::HTTP_11, reqwest::Version::HTTP_2]).await;
    }

    #[tokio::test]
    async fn tower_custom_make_service() {
        let builder = HttpServer::builder().custom_tower_make_service_factory(
            tower::service_fn(|target| async move {
                assert_eq!(target, 42);
                Ok::<_, Infallible>(tower::service_fn(|_| async move {
                    Ok::<_, Infallible>(http::Response::new(RESPONSE_TEXT.to_string()))
                }))
            }),
            42,
        );

        test_server(builder, &[reqwest::Version::HTTP_11, reqwest::Version::HTTP_2]).await;
    }

    #[tokio::test]
    async fn tower_make_service_with_addr() {
        let builder = HttpServer::builder().tower_make_service_with_addr(tower::service_fn(|addr: SocketAddr| async move {
            assert!(addr.ip().is_loopback());
            Ok::<_, Infallible>(tower::service_fn(|_| async move {
                Ok::<_, Infallible>(http::Response::new(RESPONSE_TEXT.to_string()))
            }))
        }));

        test_server(builder, &[reqwest::Version::HTTP_11, reqwest::Version::HTTP_2]).await;
    }

    #[tokio::test]
    async fn axum_service() {
        let router = axum::Router::new().route(
            "/",
            axum::routing::get(|req: String| async move {
                assert_eq!(req, RESPONSE_TEXT);
                http::Response::new(RESPONSE_TEXT.to_string())
            }),
        );

        let builder = HttpServer::builder().tower_make_service_factory(router.into_make_service());

        test_server(builder, &[reqwest::Version::HTTP_11, reqwest::Version::HTTP_2]).await;
    }
}
