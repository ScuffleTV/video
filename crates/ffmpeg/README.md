# scuffle-ffmpeg

> [!WARNING]  
> This crate is under active development and may not be stable.

[![crates.io](https://img.shields.io/crates/v/scuffle-ffmpeg.svg)](https://crates.io/crates/scuffle-ffmpeg) [![docs.rs](https://img.shields.io/docsrs/scuffle-ffmpeg)](https://docs.rs/scuffle-ffmpeg)

---

A crate designed to provide a simple interface to the native ffmpeg c-bindings.

Currently this crate only supports the latest versions of ffmpeg (7.x.x)

## Why do we need this?

This crate aims to provide a simple-safe interface to the native ffmpeg c-bindings.

## How is this different from other ffmpeg crates?

The other main ffmpeg crate is [ffmpeg-next](https://github.com/zmwangx/rust-ffmpeg).

This crate adds a few features and has a safer API. Notably it adds the ability to provide a an in-memory decode / encode buffer.

## Status

This crate is currently under development and is not yet stable.

Unit tests are not yet fully implemented. Use at your own risk.

## License

This project is licensed under the [MIT](./LICENSE.MIT) or [Apache-2.0](./LICENSE.Apache-2.0) license.
You can choose between one of them if you use this work.

`SPDX-License-Identifier: MIT OR Apache-2.0`
