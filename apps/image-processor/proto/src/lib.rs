#![allow(clippy::all)]

tonic::include_proto!("scuffle.image_processor");

#[cfg(feature = "serde")]
include!(concat!(env!("OUT_DIR"), "/scuffle.image_processor.serde.rs"));
