use std::borrow::Cow;

use opentelemetry::KeyValue;

/// A helper trait to force the compiler to check that the collector is valid.
#[doc(hidden)]
pub trait IsCollector: private::Sealed {
	type Builder<'a>;

	fn builder(meter: &opentelemetry::metrics::Meter, name: impl Into<Cow<'static, str>>) -> Self::Builder<'_>;
}

mod private {
	pub trait Sealed {
		type Value;
	}
}

macro_rules! impl_collector {
	($t:ty, $value:ty, $func:ident, $builder:ty) => {
		impl private::Sealed for $t {
			type Value = $value;
		}

		impl IsCollector for $t {
			type Builder<'a> = $builder;

			fn builder(meter: &opentelemetry::metrics::Meter, name: impl Into<Cow<'static, str>>) -> Self::Builder<'_> {
				meter.$func(name)
			}
		}
	};
}

/// A counter metric. Alias for `opentelemetry::metrics::Counter<T>`.
///
/// Counter metrics are used to record a value that can only increase.
pub type Counter<T> = opentelemetry::metrics::Counter<T>;

/// A counter metric with a `f64` value.
///
/// Counter metrics are used to record a value that can only increase.
pub type CounterF64 = Counter<f64>;

/// A counter metric with a `u64` value.
///
/// Counter metrics are used to record a value that can only increase.
pub type CounterU64 = Counter<u64>;

impl_collector!(
	CounterF64,
	f64,
	f64_counter,
	opentelemetry::metrics::InstrumentBuilder<'a, CounterF64>
);
impl_collector!(
	CounterU64,
	u64,
	u64_counter,
	opentelemetry::metrics::InstrumentBuilder<'a, CounterU64>
);

/// A gauge metric. Alias for `opentelemetry::metrics::Gauge<T>`.
/// Gauge metrics are used to record a value at the current time, and are not
/// aggregated. If you need to record a value that can be aggregated, use a
/// `Counter` or `UpDownCounter` instead.
pub type Gauge<T> = opentelemetry::metrics::Gauge<T>;

/// A gauge metric with a `f64` value.
///
/// Gauge metrics are used to record a value at the current time, and are not
/// aggregated. If you need to record a value that can be aggregated, use a
/// `Counter` or `UpDownCounter` instead.
pub type GaugeF64 = Gauge<f64>;

/// A gauge metric with a `i64` value.
///
/// Gauge metrics are used to record a value at the current time, and are not
/// aggregated. If you need to record a value that can be aggregated, use a
/// `Counter` or `UpDownCounter` instead.
pub type GaugeI64 = Gauge<i64>;

/// A gauge metric with a `u64` value.
///
/// Gauge metrics are used to record a value at the current time, and are not
/// aggregated. If you need to record a value that can be aggregated, use a
/// `Counter` or `UpDownCounter` instead.
pub type GaugeU64 = Gauge<u64>;

impl_collector!(
	GaugeF64,
	f64,
	f64_gauge,
	opentelemetry::metrics::InstrumentBuilder<'a, GaugeF64>
);
impl_collector!(
	GaugeI64,
	i64,
	i64_gauge,
	opentelemetry::metrics::InstrumentBuilder<'a, GaugeI64>
);
impl_collector!(
	GaugeU64,
	u64,
	u64_gauge,
	opentelemetry::metrics::InstrumentBuilder<'a, GaugeU64>
);

/// A histogram metric. Alias for `opentelemetry::metrics::Histogram<T>`.
///
/// Histograms are used to record a distribution of values.
pub type Histogram<T> = opentelemetry::metrics::Histogram<T>;

/// A histogram metric with a `f64` value.
///
/// Histograms are used to record a distribution of values.
pub type HistogramF64 = Histogram<f64>;

/// A histogram metric with a `u64` value.
///
/// Histograms are used to record a distribution of values.
pub type HistogramU64 = Histogram<u64>;

impl private::Sealed for HistogramF64 {
	type Value = f64;
}

/// Default boundaries for a histogram in Golang.
const DEFAULT_BOUNDARIES: [f64; 11] = [0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0];

impl IsCollector for HistogramF64 {
	type Builder<'a> = opentelemetry::metrics::HistogramBuilder<'a, HistogramF64>;

	fn builder(meter: &opentelemetry::metrics::Meter, name: impl Into<Cow<'static, str>>) -> Self::Builder<'_> {
		meter.f64_histogram(name).with_boundaries(DEFAULT_BOUNDARIES.into())
	}
}

impl private::Sealed for HistogramU64 {
	type Value = u64;
}

impl IsCollector for HistogramU64 {
	type Builder<'a> = opentelemetry::metrics::HistogramBuilder<'a, HistogramU64>;

	fn builder(meter: &opentelemetry::metrics::Meter, name: impl Into<Cow<'static, str>>) -> Self::Builder<'_> {
		meter.u64_histogram(name).with_boundaries(DEFAULT_BOUNDARIES.into())
	}
}

/// A updown counter metric. Alias for
/// `opentelemetry::metrics::UpDownCounter<T>`.
///
/// UpDownCounter like the `Counter` metric, but can also decrement.
pub type UpDownCounter<T> = opentelemetry::metrics::UpDownCounter<T>;

/// A updown counter metric with a `i64` value.
///
/// UpDownCounter like the `Counter` metric, but can also decrement.
pub type UpDownCounterI64 = UpDownCounter<i64>;

/// A updown counter metric with a `f64` value.
///
/// UpDownCounter like the `Counter` metric, but can also decrement.
pub type UpDownCounterF64 = UpDownCounter<f64>;

impl_collector!(
	UpDownCounterI64,
	i64,
	i64_up_down_counter,
	opentelemetry::metrics::InstrumentBuilder<'a, UpDownCounterI64>
);
impl_collector!(
	UpDownCounterF64,
	f64,
	f64_up_down_counter,
	opentelemetry::metrics::InstrumentBuilder<'a, UpDownCounterF64>
);

/// Helper trait to get a value of one for a number type.
/// Used by the macros below to increment and decrement counters.
trait Number {
	const ONE: Self;
}

impl Number for f64 {
	const ONE: Self = 1.0;
}

impl Number for u64 {
	const ONE: Self = 1;
}

impl Number for i64 {
	const ONE: Self = 1;
}

/// A collector is a wrapper around a metric with some attributes.
#[must_use = "Collectors do nothing by themselves, you must call them"]
pub struct Collector<'a, T: IsCollector> {
	attributes: Vec<KeyValue>,
	collector: &'a T,
}

impl<'a, T: IsCollector> Collector<'a, T> {
	pub fn new(attributes: Vec<KeyValue>, collector: &'a T) -> Self {
		Self { attributes, collector }
	}

	pub fn inner(&self) -> &'a T {
		self.collector
	}
}

macro_rules! impl_counter {
	($t:ty) => {
		impl<'a> Collector<'a, opentelemetry::metrics::Counter<$t>> {
			/// Increments the counter by one.
			#[inline]
			pub fn incr(&self) {
				self.incr_by(<$t as Number>::ONE);
			}

			/// Increments the counter by the given value.
			pub fn incr_by(&self, value: $t) {
				self.collector.add(value, &self.attributes);
			}
		}
	};
}

impl_counter!(u64);
impl_counter!(f64);

macro_rules! impl_gauge {
	($t:ty) => {
		impl<'a> Collector<'a, opentelemetry::metrics::Gauge<$t>> {
			/// Sets the value of the gauge.
			pub fn record(&self, value: $t) {
				self.collector.record(value, &self.attributes);
			}
		}
	};
}

impl_gauge!(u64);
impl_gauge!(f64);
impl_gauge!(i64);

macro_rules! impl_histogram {
	($t:ty) => {
		impl<'a> Collector<'a, opentelemetry::metrics::Histogram<$t>> {
			/// Observes a new value.
			pub fn observe(&self, value: $t) {
				self.collector.record(value, &self.attributes);
			}
		}
	};
}

impl_histogram!(u64);
impl_histogram!(f64);

macro_rules! impl_updowncounter {
	($t:ty) => {
		impl<'a> Collector<'a, opentelemetry::metrics::UpDownCounter<$t>> {
			/// Increments the counter by one.
			pub fn incr(&self) {
				self.incr_by(<$t as Number>::ONE);
			}

			/// Increments the counter by the given value.
			pub fn incr_by(&self, value: $t) {
				self.collector.add(value, &self.attributes);
			}

			/// Decrements the counter by one.
			pub fn decr(&self) {
				self.decr_by(<$t as Number>::ONE);
			}

			/// Decrements the counter by the given value.
			pub fn decr_by(&self, value: $t) {
				self.collector.add(-value, &self.attributes);
			}
		}
	};
}

impl_updowncounter!(i64);
impl_updowncounter!(f64);
