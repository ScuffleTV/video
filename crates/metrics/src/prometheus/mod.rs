use std::borrow::Cow;
use std::sync::Arc;

use opentelemetry::{otel_error, otel_warn, InstrumentationScope, KeyValue};
use opentelemetry_sdk::metrics::data::{Gauge, Histogram, ResourceMetrics, Sum};
use opentelemetry_sdk::metrics::reader::MetricReader;
use opentelemetry_sdk::metrics::{ManualReader, ManualReaderBuilder};
use opentelemetry_sdk::Resource;
use prometheus_client::encoding::{EncodeCounterValue, EncodeGaugeValue};
use prometheus_client::metrics::MetricType;
use prometheus_client::registry::Unit;

#[derive(Debug, Clone)]
pub struct PrometheusExporter {
	reader: Arc<ManualReader>,
	prometheus_full_utf8: bool,
}

impl PrometheusExporter {
	pub fn builder() -> PrometheusExporterBuilder {
		PrometheusExporterBuilder::default()
	}

	pub fn collector(&self) -> Box<dyn prometheus_client::collector::Collector> {
		Box::new(self.clone())
	}
}

impl MetricReader for PrometheusExporter {
	fn register_pipeline(&self, pipeline: std::sync::Weak<opentelemetry_sdk::metrics::Pipeline>) {
		self.reader.register_pipeline(pipeline)
	}

	fn collect(
		&self,
		rm: &mut opentelemetry_sdk::metrics::data::ResourceMetrics,
	) -> opentelemetry_sdk::metrics::MetricResult<()> {
		self.reader.collect(rm)
	}

	fn force_flush(&self) -> opentelemetry_sdk::metrics::MetricResult<()> {
		self.reader.force_flush()
	}

	fn shutdown(&self) -> opentelemetry_sdk::metrics::MetricResult<()> {
		self.reader.shutdown()
	}

	fn temporality(&self, kind: opentelemetry_sdk::metrics::InstrumentKind) -> opentelemetry_sdk::metrics::Temporality {
		self.reader.temporality(kind)
	}
}

#[derive(Default)]
pub struct PrometheusExporterBuilder {
	reader: ManualReaderBuilder,
	prometheus_full_utf8: bool,
}

impl PrometheusExporterBuilder {
	pub fn with_temporality(mut self, temporality: opentelemetry_sdk::metrics::Temporality) -> Self {
		self.reader = self.reader.with_temporality(temporality);
		self
	}

	/// Allow full UTF-8 labels in Prometheus.
	///
	/// This is disabled by default however if you are using a newer version of
	/// Prometheus that supports full UTF-8 labels you may enable this feature.
	pub fn with_prometheus_full_utf8(mut self, prometheus_full_utf8: bool) -> Self {
		self.prometheus_full_utf8 = prometheus_full_utf8;
		self
	}

	pub fn build(self) -> PrometheusExporter {
		PrometheusExporter {
			reader: Arc::new(self.reader.build()),
			prometheus_full_utf8: self.prometheus_full_utf8,
		}
	}
}

pub fn exporter() -> PrometheusExporterBuilder {
	PrometheusExporter::builder()
}

#[derive(Debug, Clone, Copy)]
enum RawNumber {
	U64(u64),
	I64(i64),
	F64(f64),
	#[cfg(feature = "extended-numbers")]
	U32(u32),
	#[cfg(feature = "extended-numbers")]
	U16(u16),
	#[cfg(feature = "extended-numbers")]
	U8(u8),
	#[cfg(feature = "extended-numbers")]
	I32(i32),
	#[cfg(feature = "extended-numbers")]
	I16(i16),
	#[cfg(feature = "extended-numbers")]
	I8(i8),
	#[cfg(feature = "extended-numbers")]
	F32(f32),
}

impl RawNumber {
	fn as_f64(&self) -> f64 {
		match *self {
			RawNumber::U64(value) => value as f64,
			RawNumber::I64(value) => value as f64,
			RawNumber::F64(value) => value,
			#[cfg(feature = "extended-numbers")]
			RawNumber::U32(value) => value as f64,
			#[cfg(feature = "extended-numbers")]
			RawNumber::U16(value) => value as f64,
			#[cfg(feature = "extended-numbers")]
			RawNumber::U8(value) => value as f64,
			#[cfg(feature = "extended-numbers")]
			RawNumber::I32(value) => value as f64,
			#[cfg(feature = "extended-numbers")]
			RawNumber::I16(value) => value as f64,
			#[cfg(feature = "extended-numbers")]
			RawNumber::I8(value) => value as f64,
			#[cfg(feature = "extended-numbers")]
			RawNumber::F32(value) => value as f64,
		}
	}
}

impl EncodeGaugeValue for RawNumber {
	fn encode(&self, encoder: &mut prometheus_client::encoding::GaugeValueEncoder) -> Result<(), std::fmt::Error> {
		match *self {
			RawNumber::U64(value) => EncodeGaugeValue::encode(&(value as i64), encoder),
			RawNumber::I64(value) => EncodeGaugeValue::encode(&value, encoder),
			RawNumber::F64(value) => EncodeGaugeValue::encode(&value, encoder),
			#[cfg(feature = "extended-numbers")]
			RawNumber::U32(value) => EncodeGaugeValue::encode(&value, encoder),
			#[cfg(feature = "extended-numbers")]
			RawNumber::U16(value) => EncodeGaugeValue::encode(&(value as u32), encoder),
			#[cfg(feature = "extended-numbers")]
			RawNumber::U8(value) => EncodeGaugeValue::encode(&(value as u32), encoder),
			#[cfg(feature = "extended-numbers")]
			RawNumber::I32(value) => EncodeGaugeValue::encode(&(value as i64), encoder),
			#[cfg(feature = "extended-numbers")]
			RawNumber::I16(value) => EncodeGaugeValue::encode(&(value as i64), encoder),
			#[cfg(feature = "extended-numbers")]
			RawNumber::I8(value) => EncodeGaugeValue::encode(&(value as i64), encoder),
			#[cfg(feature = "extended-numbers")]
			RawNumber::F32(value) => EncodeGaugeValue::encode(&(value as f64), encoder),
		}
	}
}

impl EncodeCounterValue for RawNumber {
	fn encode(&self, encoder: &mut prometheus_client::encoding::CounterValueEncoder) -> Result<(), std::fmt::Error> {
		match *self {
			RawNumber::U64(value) => EncodeCounterValue::encode(&value, encoder),
			RawNumber::I64(value) => EncodeCounterValue::encode(&(value as f64), encoder),
			RawNumber::F64(value) => EncodeCounterValue::encode(&value, encoder),
			#[cfg(feature = "extended-numbers")]
			RawNumber::U32(value) => EncodeCounterValue::encode(&(value as u64), encoder),
			#[cfg(feature = "extended-numbers")]
			RawNumber::U16(value) => EncodeCounterValue::encode(&(value as u64), encoder),
			#[cfg(feature = "extended-numbers")]
			RawNumber::U8(value) => EncodeCounterValue::encode(&(value as u64), encoder),
			#[cfg(feature = "extended-numbers")]
			RawNumber::I32(value) => EncodeCounterValue::encode(&(value as f64), encoder),
			#[cfg(feature = "extended-numbers")]
			RawNumber::I16(value) => EncodeCounterValue::encode(&(value as f64), encoder),
			#[cfg(feature = "extended-numbers")]
			RawNumber::I8(value) => EncodeCounterValue::encode(&(value as f64), encoder),
			#[cfg(feature = "extended-numbers")]
			RawNumber::F32(value) => EncodeCounterValue::encode(&(value as f64), encoder),
		}
	}
}

macro_rules! impl_raw_number {
	($t:ty, $variant:ident) => {
		impl From<$t> for RawNumber {
			fn from(value: $t) -> Self {
				RawNumber::$variant(value)
			}
		}
	};
}

impl_raw_number!(u64, U64);
impl_raw_number!(i64, I64);
impl_raw_number!(f64, F64);

#[cfg(feature = "extended-numbers")]
const _: () = {
	impl_raw_number!(u32, U32);
	impl_raw_number!(u16, U16);
	impl_raw_number!(u8, U8);
	impl_raw_number!(i32, I32);
	impl_raw_number!(i16, I16);
	impl_raw_number!(i8, I8);
	impl_raw_number!(f32, F32);
};

enum KnownMetricT<'a, T> {
	Gauge(&'a Gauge<T>),
	Sum(&'a Sum<T>),
	Histogram(&'a Histogram<T>),
}

impl<'a, T: 'static> KnownMetricT<'a, T>
where
	RawNumber: From<T>,
	T: Copy,
{
	fn from_any(any: &'a dyn std::any::Any) -> Option<Self> {
		if let Some(gauge) = any.downcast_ref::<Gauge<T>>() {
			Some(KnownMetricT::Gauge(gauge))
		} else if let Some(sum) = any.downcast_ref::<Sum<T>>() {
			Some(KnownMetricT::Sum(sum))
		} else {
			any.downcast_ref::<Histogram<T>>()
				.map(|histogram| KnownMetricT::Histogram(histogram))
		}
	}

	fn metric_type(&self) -> MetricType {
		match self {
			KnownMetricT::Gauge(_) => MetricType::Gauge,
			KnownMetricT::Sum(sum) => {
				if sum.is_monotonic {
					MetricType::Counter
				} else {
					MetricType::Gauge
				}
			}
			KnownMetricT::Histogram(_) => MetricType::Histogram,
		}
	}

	fn encode(
		&self,
		mut encoder: prometheus_client::encoding::MetricEncoder,
		labels: KeyValueEncoder<'a>,
	) -> Result<(), std::fmt::Error> {
		match self {
			KnownMetricT::Gauge(gauge) => {
				for data_point in &gauge.data_points {
					let number = RawNumber::from(data_point.value);
					encoder
						.encode_family(&labels.with_attrs(Some(&data_point.attributes)))?
						.encode_gauge(&number)?;
				}
			}
			KnownMetricT::Sum(sum) => {
				for data_point in &sum.data_points {
					let number = RawNumber::from(data_point.value);
					let attrs = labels.with_attrs(Some(&data_point.attributes));
					let mut encoder = encoder.encode_family(&attrs)?;

					if sum.is_monotonic {
						// TODO(troy): Exemplar support
						encoder.encode_counter::<(), _, f64>(&number, None)?;
					} else {
						encoder.encode_gauge(&number)?;
					}
				}
			}
			KnownMetricT::Histogram(histogram) => {
				for data_point in &histogram.data_points {
					let attrs = labels.with_attrs(Some(&data_point.attributes));
					let mut encoder = encoder.encode_family(&attrs)?;

					let sum = RawNumber::from(data_point.sum);

					let buckets = data_point
						.bounds
						.iter()
						.copied()
						.zip(data_point.bucket_counts.iter().copied())
						.collect::<Vec<_>>();

					encoder.encode_histogram::<()>(sum.as_f64(), data_point.count, &buckets, None)?;
				}
			}
		}

		Ok(())
	}
}

enum KnownMetric<'a> {
	U64(KnownMetricT<'a, u64>),
	I64(KnownMetricT<'a, i64>),
	F64(KnownMetricT<'a, f64>),
	#[cfg(feature = "extended-numbers")]
	U32(KnownMetricT<'a, u32>),
	#[cfg(feature = "extended-numbers")]
	U16(KnownMetricT<'a, u16>),
	#[cfg(feature = "extended-numbers")]
	U8(KnownMetricT<'a, u8>),
	#[cfg(feature = "extended-numbers")]
	I32(KnownMetricT<'a, i32>),
	#[cfg(feature = "extended-numbers")]
	I16(KnownMetricT<'a, i16>),
	#[cfg(feature = "extended-numbers")]
	I8(KnownMetricT<'a, i8>),
	#[cfg(feature = "extended-numbers")]
	F32(KnownMetricT<'a, f32>),
}

impl<'a> KnownMetric<'a> {
	fn from_any(any: &'a dyn std::any::Any) -> Option<Self> {
		macro_rules! try_decode {
			($t:ty, $variant:ident) => {
				if let Some(metric) = KnownMetricT::<$t>::from_any(any) {
					return Some(KnownMetric::$variant(metric));
				}
			};
		}

		try_decode!(u64, U64);
		try_decode!(i64, I64);
		try_decode!(f64, F64);
		#[cfg(feature = "extended-numbers")]
		try_decode!(u32, U32);
		#[cfg(feature = "extended-numbers")]
		try_decode!(i32, I32);
		#[cfg(feature = "extended-numbers")]
		try_decode!(u16, U16);
		#[cfg(feature = "extended-numbers")]
		try_decode!(i16, I16);
		#[cfg(feature = "extended-numbers")]
		try_decode!(u8, U8);
		#[cfg(feature = "extended-numbers")]
		try_decode!(i8, I8);
		#[cfg(feature = "extended-numbers")]
		try_decode!(f32, F32);

		None
	}

	fn metric_type(&self) -> MetricType {
		match self {
			KnownMetric::U64(metric) => metric.metric_type(),
			KnownMetric::I64(metric) => metric.metric_type(),
			KnownMetric::F64(metric) => metric.metric_type(),
			#[cfg(feature = "extended-numbers")]
			KnownMetric::U32(metric) => metric.metric_type(),
			#[cfg(feature = "extended-numbers")]
			KnownMetric::I32(metric) => metric.metric_type(),
			#[cfg(feature = "extended-numbers")]
			KnownMetric::U16(metric) => metric.metric_type(),
			#[cfg(feature = "extended-numbers")]
			KnownMetric::I16(metric) => metric.metric_type(),
			#[cfg(feature = "extended-numbers")]
			KnownMetric::U8(metric) => metric.metric_type(),
			#[cfg(feature = "extended-numbers")]
			KnownMetric::I8(metric) => metric.metric_type(),
			#[cfg(feature = "extended-numbers")]
			KnownMetric::F32(metric) => metric.metric_type(),
		}
	}

	fn encode(
		&self,
		encoder: prometheus_client::encoding::MetricEncoder,
		labels: KeyValueEncoder<'a>,
	) -> Result<(), std::fmt::Error> {
		match self {
			KnownMetric::U64(metric) => metric.encode(encoder, labels),
			KnownMetric::I64(metric) => metric.encode(encoder, labels),
			KnownMetric::F64(metric) => metric.encode(encoder, labels),
			#[cfg(feature = "extended-numbers")]
			KnownMetric::U32(metric) => metric.encode(encoder, labels),
			#[cfg(feature = "extended-numbers")]
			KnownMetric::I32(metric) => metric.encode(encoder, labels),
			#[cfg(feature = "extended-numbers")]
			KnownMetric::U16(metric) => metric.encode(encoder, labels),
			#[cfg(feature = "extended-numbers")]
			KnownMetric::I16(metric) => metric.encode(encoder, labels),
			#[cfg(feature = "extended-numbers")]
			KnownMetric::U8(metric) => metric.encode(encoder, labels),
			#[cfg(feature = "extended-numbers")]
			KnownMetric::I8(metric) => metric.encode(encoder, labels),
			#[cfg(feature = "extended-numbers")]
			KnownMetric::F32(metric) => metric.encode(encoder, labels),
		}
	}
}

impl prometheus_client::collector::Collector for PrometheusExporter {
	fn encode(&self, mut encoder: prometheus_client::encoding::DescriptorEncoder) -> Result<(), std::fmt::Error> {
		let mut metrics = ResourceMetrics {
			resource: Resource::empty(),
			scope_metrics: vec![],
		};

		if let Err(err) = self.reader.collect(&mut metrics) {
			otel_error!(name: "prometheus_collector_collect_error", error = err.to_string());
			return Err(std::fmt::Error);
		}

		let labels = KeyValueEncoder::new(self.prometheus_full_utf8);

		encoder
			.encode_descriptor("target", "Information about the target", None, MetricType::Info)?
			.encode_info(&labels.with_resource(Some(&metrics.resource)))?;

		for scope_metrics in &metrics.scope_metrics {
			for metric in &scope_metrics.metrics {
				let Some(known_metric) = KnownMetric::from_any(metric.data.as_any()) else {
					otel_warn!(name: "prometheus_collector_unknown_metric_type", metric_name = metric.name.as_ref());
					continue;
				};

				let unit = if metric.unit.is_empty() {
					None
				} else {
					Some(Unit::Other(metric.unit.to_string()))
				};

				known_metric.encode(
					encoder.encode_descriptor(
						&metric.name,
						&metric.description,
						unit.as_ref(),
						known_metric.metric_type(),
					)?,
					labels.with_scope(Some(&scope_metrics.scope)),
				)?;
			}
		}

		Ok(())
	}
}

fn scope_to_iter(scope: &InstrumentationScope) -> impl Iterator<Item = (&str, Cow<'_, str>)> {
	[
		("otel.scope.name", Some(Cow::Borrowed(scope.name()))),
		("otel.scope.version", scope.version().map(Cow::Borrowed)),
		("otel.scope.schema_url", scope.schema_url().map(Cow::Borrowed)),
	]
	.into_iter()
	.chain(scope.attributes().map(|kv| (kv.key.as_str(), Some(kv.value.as_str()))))
	.filter_map(|(key, value)| value.map(|v| (key, v)))
}

#[derive(Debug, Clone, Copy)]
struct KeyValueEncoder<'a> {
	resource: Option<&'a Resource>,
	scope: Option<&'a InstrumentationScope>,
	attrs: Option<&'a [KeyValue]>,
	prometheus_full_utf8: bool,
}

impl<'a> KeyValueEncoder<'a> {
	fn new(prometheus_full_utf8: bool) -> Self {
		Self {
			resource: None,
			scope: None,
			attrs: None,
			prometheus_full_utf8,
		}
	}

	pub fn with_resource(self, resource: Option<&'a Resource>) -> Self {
		Self { resource, ..self }
	}

	pub fn with_scope(self, scope: Option<&'a InstrumentationScope>) -> Self {
		Self { scope, ..self }
	}

	pub fn with_attrs(self, attrs: Option<&'a [KeyValue]>) -> Self {
		Self { attrs, ..self }
	}
}

fn escape_key(s: &str) -> Cow<'_, str> {
	// prefix chars to add in case name starts with number
	let mut prefix = "";

	// Find first invalid char
	if let Some((replace_idx, _)) = s.char_indices().find(|(i, c)| {
		if *i == 0 && c.is_ascii_digit() {
			// first char is number, add prefix and replace reset of chars
			prefix = "_";
			true
		} else {
			// keep checking
			!c.is_alphanumeric() && *c != '_' && *c != ':'
		}
	}) {
		// up to `replace_idx` have been validated, convert the rest
		let (valid, rest) = s.split_at(replace_idx);
		Cow::Owned(
			prefix
				.chars()
				.chain(valid.chars())
				.chain(rest.chars().map(|c| {
					if c.is_ascii_alphanumeric() || c == '_' || c == ':' {
						c
					} else {
						'_'
					}
				}))
				.collect(),
		)
	} else {
		Cow::Borrowed(s) // no invalid chars found, return existing
	}
}

impl prometheus_client::encoding::EncodeLabelSet for KeyValueEncoder<'_> {
	fn encode(&self, mut encoder: prometheus_client::encoding::LabelSetEncoder) -> Result<(), std::fmt::Error> {
		use std::fmt::Write;

		fn write_kv(
			encoder: &mut prometheus_client::encoding::LabelSetEncoder,
			key: &str,
			value: &str,
			prometheus_full_utf8: bool,
		) -> Result<(), std::fmt::Error> {
			let mut label = encoder.encode_label();
			let mut key_encoder = label.encode_label_key()?;
			if prometheus_full_utf8 {
				// TODO(troy): I am not sure if this is correct.
				// See: https://github.com/prometheus/client_rust/issues/251
				write!(&mut key_encoder, "{}", key)?;
			} else {
				write!(&mut key_encoder, "{}", escape_key(key))?;
			}

			let mut value_encoder = key_encoder.encode_label_value()?;
			write!(&mut value_encoder, "{}", value)?;

			value_encoder.finish()
		}

		if let Some(resource) = self.resource {
			for (key, value) in resource.iter() {
				write_kv(&mut encoder, key.as_str(), value.as_str().as_ref(), self.prometheus_full_utf8)?;
			}
		}

		if let Some(scope) = self.scope {
			for (key, value) in scope_to_iter(scope) {
				write_kv(&mut encoder, key, value.as_ref(), self.prometheus_full_utf8)?;
			}
		}

		if let Some(attrs) = self.attrs {
			for kv in attrs {
				write_kv(
					&mut encoder,
					kv.key.as_str(),
					kv.value.as_str().as_ref(),
					self.prometheus_full_utf8,
				)?;
			}
		}

		Ok(())
	}
}
