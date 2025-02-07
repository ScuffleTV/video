//! A crate designed to provide a simple interface to the native ffmpeg c-bindings.
//!
//! ## Why do we need this?
//!
//! This crate aims to provide a simple-safe interface to the native ffmpeg c-bindings.
//!
//! Currently this crate only supports the latest versions of ffmpeg (7.x.x)
//!
//! ## How is this different from other ffmpeg crates?
//!
//! The other main ffmpeg crate is [ffmpeg-next](https://github.com/zmwangx/rust-ffmpeg).
//!
//! This crate adds a few features and has a safer API. Notably it adds the ability to provide a an in-memory decode / encode buffer.
//!
//! ## Status
//!
//! This crate is currently under development and is not yet stable.
//!
//! Unit tests are not yet fully implemented. Use at your own risk.
//!
//! ## License
//!
//! This project is licensed under the [MIT](./LICENSE.MIT) or [Apache-2.0](./LICENSE.Apache-2.0) license.
//! You can choose between one of them if you use this work.
//!
//! `SPDX-License-Identifier: MIT OR Apache-2.0`
#![cfg_attr(all(coverage_nightly, test), feature(coverage_attribute))]
#![deny(missing_docs)]
#![deny(clippy::undocumented_unsafe_blocks)]
#![deny(clippy::multiple_unsafe_ops_per_block)]
#![deny(clippy::missing_const_for_fn)]

/// Codec specific functionality.
pub mod codec;
/// Constants.
pub mod consts;
/// Decoder specific functionality.
pub mod decoder;
/// Dictionary specific functionality.
pub mod dict;
/// Encoder specific functionality.
pub mod encoder;
/// Error handling.
pub mod error;
/// Filter graph specific functionality.
pub mod filter_graph;
/// Frame specific functionality.
pub mod frame;
/// Input/Output specific functionality.
pub mod io;
/// Limiter specific functionality.
pub mod limiter;
/// Logging specific functionality.
pub mod log;
/// Packet specific functionality.
pub mod packet;
/// Scalar specific functionality.
pub mod scalar;
/// Stream specific functionality.
pub mod stream;
/// Utility functionality.
pub mod utils;

/// The ffi module.
pub use ffmpeg_sys_next as ffi;

mod smart_object;
