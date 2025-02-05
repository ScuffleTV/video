# scuffle-http

> [!WARNING]  
> This crate is under active development and may not be stable.

[![crates.io](https://img.shields.io/crates/v/scuffle-http.svg)](https://crates.io/crates/scuffle-http) [![docs.rs](https://img.shields.io/docsrs/scuffle-http)](https://docs.rs/scuffle-http)

---

An HTTP server with support for HTTP/1, HTTP/2 and HTTP/3.

It abstracts away [`hyper`](https://crates.io/crates/hyper) and [`h3`](https://crates.io/crates/h3) to provide a rather simple interface for creating and running a server that can handle all three protocols.

See the [examples](./examples) directory for usage examples.

## Why do we need this?

This crate is designed to be a simple and easy to use HTTP server that supports HTTP/1, HTTP/2 and HTTP/3.

Currently, there are simply no other crates that provide support for all three protocols with a unified API.
This crate aims to fill that gap.

## Feature Flags

- `tower`: Enables support for [`tower`](https://crates.io/crates/tower) services. Enabled by default.
- `http1`: Enables support for HTTP/1. Enabled by default.
- `http2`: Enables support for HTTP/2. Enabled by default.
- `http3`: Enables support for HTTP/3. Disabled by default.
- `tracing`: Enables logging with [`tracing`](https://crates.io/crates/tracing). Disabled by default.
- `tls-rustls`: Enables support for TLS with [`rustls`](https://crates.io/crates/rustls). Disabled by default.

## Example

The following example demonstrates how to create a simple HTTP server (without TLS) that responds with "Hello, world!" to all requests on port 3000.

```rust
# use scuffle_future_ext::FutureExt;
# tokio_test::block_on(async {
# let run = async {
let service = scuffle_http::service::fn_http_service(|req| async move {
    scuffle_http::http::Response::builder()
    .status(scuffle_http::http::StatusCode::OK)
    .header(scuffle_http::http::header::CONTENT_TYPE, "text/plain")
    .body("Hello, world!".to_string())
});
let service_factory = scuffle_http::service::service_clone_factory(service);

scuffle_http::HttpServer::builder()
    .with_service_factory(service_factory)
    .bind("[::]:3000".parse().unwrap())
    .build()
    .expect("failed to build server")
    .run()
    .await
    .expect("server failed");
# };
# run.with_timeout(std::time::Duration::from_secs(1)).await.expect_err("test should have timed out");
# });
```

## Status

This crate is currently under development and is not yet stable.

### Missing Features

- HTTP/3 webtransport support
- Upgrading to websocket connections from HTTP/3 connections (this is usually done via HTTP/1.1 anyway)

Unit tests are not yet fully implemented. Use at your own risk.

## License

This project is licensed under the [MIT](./LICENSE.MIT) or [Apache-2.0](./LICENSE.Apache-2.0) license.
You can choose between one of them if you use this work.

`SPDX-License-Identifier: MIT OR Apache-2.0`
