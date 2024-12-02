use example::Kind;
use opentelemetry::KeyValue;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::Resource;

#[scuffle_metrics::metrics]
mod example {
	use scuffle_metrics::{CounterU64, MetricEnum};

	#[derive(MetricEnum)]
	pub enum Kind {
		Http,
		Grpc,
	}

	/// Requests for adding 2 numbers
	#[metrics(unit = "requests")]
	pub fn add(a: u64, b: u64, kind: Kind) -> CounterU64;
}

#[tokio::main]
async fn main() {
	let mut registry = prometheus_client::registry::Registry::default();

	let exporter = scuffle_metrics::prometheus::exporter().build();

	registry.register_collector(exporter.collector());

	let provider = SdkMeterProvider::builder()
		.with_resource(Resource::new(vec![KeyValue::new("service.name", env!("CARGO_BIN_NAME"))]))
		.with_reader(exporter)
		.build();

	opentelemetry::global::set_meter_provider(provider.clone());

	example::add(1, 2, Kind::Http).incr();

	for i in 0..10 {
		example::add(i, i, Kind::Http).incr();
	}

	let mut buffer = String::new();

	prometheus_client::encoding::text::encode(&mut buffer, &registry).unwrap();

	println!("{}", buffer);
}
