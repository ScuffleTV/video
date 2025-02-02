# scuffle-http

> [!WARNING]  
> This crate is under active development and may not be stable.

[![crates.io](https://img.shields.io/crates/v/scuffle-http.svg)](https://crates.io/crates/scuffle-http) [![docs.rs](https://img.shields.io/docsrs/scuffle-http)](https://docs.rs/scuffle-http)

---

A HTTP server with support for HTTP/1, HTTP/2 and HTTP/3.

It abstracts away [`hyper`](https://crates.io/crates/hyper) and [`h3`](https://crates.io/crates/h3) to provide a rather simple interface for creating and running a server that can handle all three protocols.

See the [examples](./examples) directory for usage examples.

## Why do we need this?

This crate is designed to be a simple and easy to use HTTP server that supports HTTP/1, HTTP/2 and HTTP/3.

Currently, there are simply no other crates that provide support for all three protocols with a unified API.
This crate aims to fill that gap.

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
