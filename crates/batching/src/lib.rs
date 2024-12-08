#![doc = include_str!("../README.md")]
#![cfg_attr(all(coverage_nightly, test), feature(coverage_attribute))]

pub mod batch;
pub mod dataloader;

pub use batch::{BatchExecutor, Batcher};
pub use dataloader::{DataLoader, DataLoaderFetcher};
