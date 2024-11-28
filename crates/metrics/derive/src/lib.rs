#![doc = include_str!("../README.md")]

use enum_impl::metric_enum_impl;
use metrics_impl::metrics_impl;
use proc_macro::TokenStream;

mod enum_impl;
mod metrics_impl;

/// A macro used to create metric handlers.
///
/// You can change the crate by specifying `#[metrics(crate = "...")]`.
///
/// Attributes:
///
/// - `crate`: The `scuffle_metrics` crate path. Valid on modules & functions.
/// - `builder`: The builder to use for the metric. Valid on functions.
/// - `unit`: The unit of the metric. Valid on functions.
/// - `rename`: The name of the metric. Valid on modules, functions & function
///   arguments.
///
/// When using the module, you do not need to attribute each function with the
/// `#[metrics]` attribute. All non function definitions are ignored.
///
/// # Module Example
///
/// ```rust
/// #[metrics]
/// mod example {
///     use scuffle_metrics::{CounterU64, MetricEnum};
///
///     #[derive(MetricEnum)]
///     pub enum Kind {
///         Http,
///         Grpc,
///     }
///
///     #[metrics(unit = "requests")]
///     pub fn request(kind: Kind) -> CounterU64;
/// }
///
/// example::request(Kind::Http).incr();
/// ```
///
/// # Function Example
///
/// ```rust
/// #[metrics(unit = "requests")]
/// pub fn request(kind: Kind) -> CounterU64;
///
/// request(Kind::Http).incr();
/// ```
#[proc_macro_attribute]
pub fn metrics(args: TokenStream, input: TokenStream) -> TokenStream {
	match metrics_impl(args, input) {
		Ok(tokens) => tokens.into(),
		Err(err) => err.to_compile_error().into(),
	}
}

/// Implements a conversion `Into<opentelemetry::Value>` for the enum.
/// This allows the enum to be used as a metric label.
#[proc_macro_derive(MetricEnum, attributes(metrics))]
pub fn metric_enum(input: TokenStream) -> TokenStream {
	match metric_enum_impl(input.into()) {
		Ok(tokens) => tokens.into(),
		Err(err) => err.to_compile_error().into(),
	}
}
