#![doc = include_str!("../README.md")]

#[cfg(feature = "prometheus")]
/// A copy of the opentelemetry-prometheus crate, updated to work with the
/// latest version of opentelemetry.
pub mod prometheus;

#[doc(hidden)]
pub mod value;

pub mod collector;

pub use collector::{
	CounterF64, CounterU64, GaugeF64, GaugeI64, GaugeU64, HistogramF64, HistogramU64, UpDownCounterF64, UpDownCounterI64,
};
pub use opentelemetry;
pub use scuffle_metrics_derive::{metrics, MetricEnum};
