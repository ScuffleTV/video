use std::collections::HashMap;
use std::str::FromStr;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use anyhow::Context;
use bson::oid::ObjectId;
use opentelemetry_otlp::WithExportConfig;
use scuffle_bootstrap_telemetry::opentelemetry::trace::TracerProvider as _;
use scuffle_bootstrap_telemetry::opentelemetry::KeyValue;
use scuffle_bootstrap_telemetry::opentelemetry_sdk::logs::LoggerProvider;
use scuffle_bootstrap_telemetry::opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider, Temporality};
use scuffle_bootstrap_telemetry::opentelemetry_sdk::trace::{Sampler, TracerProvider};
use scuffle_bootstrap_telemetry::opentelemetry_sdk::{runtime, Resource};
use scuffle_bootstrap_telemetry::{opentelemetry, opentelemetry_appender_tracing, prometheus_client, tracing_opentelemetry};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

use crate::config::{ImageProcessorConfig, LogFormat};
use crate::database::Job;
use crate::drive::public_http::PUBLIC_HTTP_DRIVE_NAME;
use crate::drive::{build_drive, AnyDrive, Drive};
use crate::event_queue::{build_event_queue, AnyEventQueue, EventQueue};

pub struct Global {
	worker_id: ObjectId,
	config: ImageProcessorConfig,
	database: mongodb::Database,
	disks: HashMap<String, AnyDrive>,
	event_queues: HashMap<String, AnyEventQueue>,
	prometheus_registry: Option<prometheus_client::registry::Registry>,
	opentelemetry: opentelemetry::OpenTelemetry,
	failed: AtomicBool,
}

impl scuffle_bootstrap::global::Global for Global {
	type Config = ImageProcessorConfig;

	async fn init(config: Self::Config) -> anyhow::Result<Arc<Self>> {
		let filter = EnvFilter::from_str(&config.level).context("invalid logging level")?;

		let mut prometheus_registry = None;
		let mut opentelemetry = opentelemetry::OpenTelemetry::new();

		let mut layers = Vec::<Box<dyn tracing_subscriber::Layer<tracing_subscriber::Registry> + Send + Sync>>::new();

		let mut default_trace = None;

		if config.telemetry.logs.enabled {
			if let Some(fmt) = config.telemetry.logs.stdout {
				let make_layer = || tracing_subscriber::fmt::layer().with_file(true).with_line_number(true);
				match fmt {
					LogFormat::Json => {
						default_trace = Some(tracing::subscriber::set_default(
							tracing_subscriber::registry().with(make_layer().json()),
						));
						layers.push(make_layer().json().boxed());
					}
					LogFormat::Text => {
						default_trace = Some(tracing::subscriber::set_default(
							tracing_subscriber::registry().with(make_layer()),
						));
						layers.push(make_layer().boxed());
					}
				}

				tracing::info!("initializing logging");
			}

			if config.telemetry.logs.push {
				let resource = Resource::new({
					let mut labels = vec![];
					if !config.telemetry.logs.labels.contains_key("service.name") {
						labels.push(KeyValue::new("service.name", env!("CARGO_BIN_NAME")));
					}

					labels.extend(
						config
							.telemetry
							.logs
							.labels
							.iter()
							.map(|(k, v)| KeyValue::new(k.clone(), v.clone())),
					);
					labels
				});

				let exporter = opentelemetry_otlp::LogExporter::builder()
					.with_tonic()
					.with_endpoint(config.telemetry.logs.otlp_endpoint.clone())
					.build()
					.context("otlp log exporter")?;

				let provider = LoggerProvider::builder()
					.with_resource(resource.clone())
					.with_batch_exporter(exporter, runtime::Tokio)
					.build();

				layers.push(opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(&provider).boxed());
				opentelemetry = opentelemetry.with_logs(provider);
			}
		}

		if config.telemetry.metrics.enabled {
			let mut registry = prometheus_client::registry::Registry::default();
			let resource = Resource::new({
				let mut labels = vec![];
				if !config.telemetry.metrics.labels.contains_key("service.name") {
					labels.push(KeyValue::new("service.name", env!("CARGO_BIN_NAME")));
				}

				labels.extend(
					config
						.telemetry
						.metrics
						.labels
						.iter()
						.map(|(k, v)| KeyValue::new(k.clone(), v.clone())),
				);
				labels
			});

			let prometheus_exporter = scuffle_metrics::prometheus::exporter().build();
			registry.register_collector(prometheus_exporter.collector());

			prometheus_registry = Some(registry);

			let mut provider = SdkMeterProvider::builder()
				.with_resource(resource.clone())
				.with_reader(prometheus_exporter);

			if config.telemetry.metrics.push {
				let exporter = opentelemetry_otlp::MetricExporter::builder()
					.with_tonic()
					.with_temporality(Temporality::Delta)
					.with_endpoint(config.telemetry.metrics.otlp_endpoint.clone())
					.build()
					.context("otlp metric exporter")?;

				provider = provider.with_reader(PeriodicReader::builder(exporter, runtime::Tokio).build());
			}

			let provider = provider.build();

			opentelemetry = opentelemetry.with_metrics(provider.clone());
			opentelemetry::global::set_meter_provider(provider);
		}

		if config.telemetry.traces.enabled {
			let resource = Resource::new({
				let mut labels = vec![];
				if !config.telemetry.traces.labels.contains_key("service.name") {
					labels.push(KeyValue::new("service.name", env!("CARGO_BIN_NAME")));
				}

				labels.extend(
					config
						.telemetry
						.traces
						.labels
						.iter()
						.map(|(k, v)| KeyValue::new(k.clone(), v.clone())),
				);
				labels
			});

			let exporter = opentelemetry_otlp::SpanExporter::builder()
				.with_tonic()
				.with_endpoint(config.telemetry.traces.otlp_endpoint.clone())
				.build()
				.context("otlp trace exporter")?;

			let provider = TracerProvider::builder()
				.with_resource(resource.clone())
				.with_sampler(Sampler::TraceIdRatioBased(config.telemetry.traces.sample_rate))
				.with_batch_exporter(exporter, runtime::Tokio)
				.build();

			opentelemetry::global::set_tracer_provider(provider.clone());
			layers.push(
				tracing_opentelemetry::layer()
					.with_tracer(provider.tracer("image-processor"))
					.boxed(),
			);
			opentelemetry = opentelemetry.with_traces(provider);
		}

		tracing_subscriber::registry()
			.with(layers.with_filter(filter))
			.try_init()
			.context("set_global_default")?;

		drop(default_trace);

		const DEFAULT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);
		tracing::debug!("setting up mongo client");

		let client = tokio::time::timeout(DEFAULT_TIMEOUT, mongodb::Client::with_uri_str(&config.database.uri))
			.await
			.context("mongodb timeout")?
			.context("mongodb")?;
		let Some(database) = client.default_database() else {
			anyhow::bail!("no default database")
		};

		tracing::debug!("setting up job collection");

		tokio::time::timeout(DEFAULT_TIMEOUT, Job::setup_collection(&database))
			.await
			.context("job collection timeout")?
			.context("job collection")?;

		tracing::debug!("setting up disks and event queues");

		let mut disks = HashMap::new();

		for disk in &config.drives {
			let disk = tokio::time::timeout(DEFAULT_TIMEOUT, build_drive(disk))
				.await
				.context("disk timeout")?
				.context("disk")?;

			let name = disk.name().to_string();
			if disks.insert(name.clone(), disk).is_some() {
				anyhow::bail!("duplicate disk name: {name}");
			}
		}

		if config.drives.is_empty() {
			tracing::warn!("no disks configured");
		}

		let mut event_queues = HashMap::new();

		for event_queue in &config.event_queues {
			let event_queue = tokio::time::timeout(DEFAULT_TIMEOUT, build_event_queue(event_queue))
				.await
				.context("event queue timeout")?
				.context("event queue")?;

			let name = event_queue.name().to_string();
			if event_queues.insert(name.clone(), event_queue).is_some() {
				anyhow::bail!("duplicate event queue name: {name}");
			}
		}

		if config.event_queues.is_empty() {
			tracing::warn!("no event queues configured");
		}

		Ok(Arc::new(Self {
			worker_id: ObjectId::new(),
			config,
			database,
			disks,
			event_queues,
			prometheus_registry,
			opentelemetry,
			failed: AtomicBool::new(false),
		}))
	}

	async fn on_services_start(self: &Arc<Self>) -> anyhow::Result<()> {
		tracing::info!("started services");
		Ok(())
	}

	async fn on_service_exit(self: &Arc<Self>, name: &'static str, result: anyhow::Result<()>) -> anyhow::Result<()> {
		if let Err(ref err) = result {
			tracing::error!("service {name} exited with error: {:#}", err);
			self.failed.store(true, std::sync::atomic::Ordering::Relaxed);
			scuffle_context::Handler::global().cancel();
		} else {
			tracing::info!("service {name} exited");
		}

		Ok(())
	}

	async fn on_exit(self: &Arc<Self>, result: anyhow::Result<()>) -> anyhow::Result<()> {
		if let Err(err) = result {
			tracing::error!("exit error: {:#}", err);
			std::process::exit(1);
		} else {
			tracing::info!("shutdown complete");
			if self.failed.load(std::sync::atomic::Ordering::Relaxed) {
				std::process::exit(1);
			} else {
				std::process::exit(0);
			}
		}
	}
}

impl scuffle_signal::SignalConfig for Global {
	async fn on_shutdown(self: &Arc<Self>) -> anyhow::Result<()> {
		tracing::info!("shutting down");
		Ok(())
	}
}

impl Global {
	pub fn worker_id(&self) -> ObjectId {
		self.worker_id
	}

	pub fn config(&self) -> &ImageProcessorConfig {
		&self.config
	}

	pub fn drive(&self, name: &str) -> Option<&AnyDrive> {
		self.disks.get(name)
	}

	pub fn drives(&self) -> &HashMap<String, AnyDrive> {
		&self.disks
	}

	pub fn event_queues(&self) -> &HashMap<String, AnyEventQueue> {
		&self.event_queues
	}

	pub fn event_queue(&self, name: &str) -> Option<&AnyEventQueue> {
		self.event_queues.get(name)
	}

	pub fn public_http_drive(&self) -> Option<&AnyDrive> {
		self.drive(PUBLIC_HTTP_DRIVE_NAME)
	}

	pub fn database(&self) -> &mongodb::Database {
		&self.database
	}
}

impl scuffle_bootstrap_telemetry::TelemetryConfig for Global {
	fn enabled(&self) -> bool {
		self.config.telemetry.logs.enabled || self.config.telemetry.metrics.enabled || self.config.telemetry.traces.enabled
	}

	fn bind_address(&self) -> Option<std::net::SocketAddr> {
		self.config.telemetry.bind
	}

	async fn health_check(&self) -> Result<(), anyhow::Error> {
		if let Err(err) = self.database().run_command(bson::doc! { "ping": 1 }).await {
			anyhow::bail!("database ping failed: {err}");
		}

		for disk in self.drives().values() {
			if !disk.healthy().await {
				anyhow::bail!("disk check failed: {}", disk.name());
			}
		}

		for event_queue in self.event_queues().values() {
			if !event_queue.healthy().await {
				anyhow::bail!("event queue check failed: {}", event_queue.name());
			}
		}

		Ok(())
	}

	fn opentelemetry(&self) -> Option<&scuffle_bootstrap_telemetry::opentelemetry::OpenTelemetry> {
		Some(&self.opentelemetry)
	}

	fn prometheus_metrics_registry(&self) -> Option<&prometheus_client::registry::Registry> {
		self.prometheus_registry.as_ref()
	}
}
