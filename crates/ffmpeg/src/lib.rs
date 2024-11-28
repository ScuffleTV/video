#![doc = include_str!("../README.md")]

pub mod codec;
pub mod consts;
pub mod decoder;
pub mod dict;
pub mod encoder;
pub mod error;
pub mod filter_graph;
pub mod frame;
pub mod io;
pub mod limiter;
pub mod log;
pub mod packet;
pub mod scalar;
pub mod stream;
pub mod utils;

pub use ffmpeg_sys_next as ffi;

mod smart_object;
