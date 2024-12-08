use anyhow::Context;
use bytes::Bytes;
#[cfg(feature = "opentelemetry-logs")]
pub use opentelemetry_appender_tracing;
#[cfg(feature = "opentelemetry")]
pub use opentelemetry_sdk;
#[cfg(feature = "prometheus")]
pub use prometheus_client;
use scuffle_bootstrap::global::Global;
use scuffle_bootstrap::service::Service;
use scuffle_context::ContextFutExt;
use scuffle_http::backend::HttpServer;
use scuffle_http::body::IncomingBody;
#[cfg(feature = "opentelemetry-traces")]
pub use tracing_opentelemetry;

pub struct TelemetrySvc;

pub trait TelemetryConfig: Global {
	/// Return true if the service is enabled.
	fn enabled(&self) -> bool {
		true
	}

	/// Return the bind address to listen on.
	fn bind_address(&self) -> Option<std::net::SocketAddr> {
		None
	}

	/// Return the http server name.
	fn http_server_name(&self) -> &str {
		"scuffle-bootstrap-telemetry"
	}

	/// Return a health check to determine if the service is healthy.
	fn health_check(&self) -> impl std::future::Future<Output = Result<(), anyhow::Error>> + Send {
		std::future::ready(Ok(()))
	}

	/// Return a Prometheus metrics registry to scrape metrics from.
	#[cfg(feature = "prometheus")]
	fn prometheus_metrics_registry(&self) -> Option<&prometheus_client::registry::Registry> {
		None
	}

	/// Pass an OpenTelemetry instance to the service.
	/// If provided the service will flush and shutdown the OpenTelemetry
	/// instance when the service shuts down.
	#[cfg(feature = "opentelemetry")]
	fn opentelemetry(&self) -> Option<&opentelemetry::OpenTelemetry> {
		None
	}
}

impl<Global: TelemetryConfig> Service<Global> for TelemetrySvc {
	async fn enabled(&self, global: &std::sync::Arc<Global>) -> anyhow::Result<bool> {
		Ok(global.enabled())
	}

	async fn run(self, global: std::sync::Arc<Global>, ctx: scuffle_context::Context) -> anyhow::Result<()> {
		let server = global.bind_address().map(|addr| {
			scuffle_http::backend::tcp::TcpServerConfig::builder()
				.with_bind(addr)
				.with_server_name(global.http_server_name())
				.build()
				.into_server()
		});

		let global2 = global.clone();

		if let Some(server) = server {
			server
				.start(
					scuffle_http::svc::function_service(move |req| {
						let global = global2.clone();
						async move {
							match req.uri().path() {
								"/health" => health_check(&global, req).await,
								#[cfg(feature = "prometheus")]
								"/metrics" => metrics(&global, req).await,
								#[cfg(feature = "pprof")]
								"/pprof/cpu" => pprof(&global, req).await,
								#[cfg(feature = "opentelemetry")]
								"/opentelemetry/flush" => opentelemetry_flush(&global).await,
								_ => Ok(http::Response::builder()
									.status(http::StatusCode::NOT_FOUND)
									.body(http_body_util::Full::new(Bytes::from_static(b"not found")))?),
							}
						}
					}),
					1,
				)
				.await
				.context("server start")?;

			server.wait().with_context(&ctx).await.transpose().context("server wait")?;

			server.shutdown().await.context("server shutdown")?;
		} else {
			ctx.done().await;
		}

		#[cfg(feature = "opentelemetry")]
		if let Some(opentelemetry) = global.opentelemetry().cloned() {
			if opentelemetry.is_enabled() {
				tokio::task::spawn_blocking(move || opentelemetry.shutdown())
					.await
					.context("opentelemetry shutdown spawn")?
					.context("opentelemetry shutdown")?;
			}
		}

		Ok(())
	}
}

async fn health_check<G: TelemetryConfig>(
	global: &std::sync::Arc<G>,
	_: http::Request<IncomingBody>,
) -> Result<http::Response<http_body_util::Full<Bytes>>, scuffle_http::Error> {
	if let Err(err) = global.health_check().await {
		tracing::error!("health check failed: {err}");
		Ok(http::Response::builder()
			.status(http::StatusCode::INTERNAL_SERVER_ERROR)
			.body(http_body_util::Full::new(format!("{err:#}").into()))?)
	} else {
		Ok(http::Response::builder()
			.status(http::StatusCode::OK)
			.body(http_body_util::Full::new(Bytes::from_static(b"ok")))?)
	}
}

#[cfg(feature = "prometheus")]
async fn metrics<G: TelemetryConfig>(
	global: &std::sync::Arc<G>,
	_: http::Request<IncomingBody>,
) -> Result<http::Response<http_body_util::Full<Bytes>>, scuffle_http::Error> {
	if let Some(metrics) = global.prometheus_metrics_registry() {
		let mut buf = String::new();
		if prometheus_client::encoding::text::encode(&mut buf, metrics).is_err() {
			tracing::error!("metrics encode failed");
			return Ok(http::Response::builder()
				.status(http::StatusCode::INTERNAL_SERVER_ERROR)
				.body(http_body_util::Full::new("metrics encode failed".to_string().into()))?);
		}

		Ok(http::Response::builder()
			.status(http::StatusCode::OK)
			.body(http_body_util::Full::new(Bytes::from(buf)))?)
	} else {
		Ok(http::Response::builder()
			.status(http::StatusCode::NOT_FOUND)
			.body(http_body_util::Full::new(Bytes::from_static(b"not found")))?)
	}
}

#[cfg(feature = "pprof")]
async fn pprof<G: TelemetryConfig>(
	_: &std::sync::Arc<G>,
	req: http::Request<IncomingBody>,
) -> Result<http::Response<http_body_util::Full<Bytes>>, scuffle_http::Error> {
	let query = req.uri().query();
	let query = query.map(querystring::querify).into_iter().flatten();

	let mut freq = 100;
	let mut duration = std::time::Duration::from_secs(5);
	let mut ignore_list = Vec::new();

	for (key, value) in query {
		if key == "freq" {
			freq = match value.parse() {
				Ok(v) => v,
				Err(err) => {
					return Ok(http::Response::builder()
						.status(http::StatusCode::BAD_REQUEST)
						.body(http_body_util::Full::new(format!("invalid freq: {err:#}").into()))?);
				}
			};
		} else if key == "duration" {
			duration = match value.parse() {
				Ok(v) => std::time::Duration::from_secs(v),
				Err(err) => {
					return Ok(http::Response::builder()
						.status(http::StatusCode::BAD_REQUEST)
						.body(http_body_util::Full::new(format!("invalid duration: {err:#}").into()))?);
				}
			};
		} else if key == "ignore" {
			ignore_list.push(value);
		}
	}

	let cpu = scuffle_pprof::Cpu::new(freq, &ignore_list);

	match tokio::task::spawn_blocking(move || cpu.capture(duration)).await {
		Ok(Ok(data)) => Ok(http::Response::builder()
			.status(http::StatusCode::OK)
			.body(http_body_util::Full::new(Bytes::from(data)))?),
		Ok(Err(err)) => {
			tracing::error!("cpu capture failed: {err:#}");
			Ok(http::Response::builder()
				.status(http::StatusCode::INTERNAL_SERVER_ERROR)
				.body(http_body_util::Full::new(format!("{err:#}").into()))?)
		}
		Err(err) => {
			tracing::error!("cpu capture failed: {err:#}");
			Ok(http::Response::builder()
				.status(http::StatusCode::INTERNAL_SERVER_ERROR)
				.body(http_body_util::Full::new(format!("{err:#}").into()))?)
		}
	}
}

#[cfg(feature = "opentelemetry")]
async fn opentelemetry_flush<G: TelemetryConfig>(
	global: &std::sync::Arc<G>,
) -> Result<http::Response<http_body_util::Full<Bytes>>, scuffle_http::Error> {
	if let Some(opentelemetry) = global.opentelemetry().cloned() {
		if opentelemetry.is_enabled() {
			match tokio::task::spawn_blocking(move || opentelemetry.flush()).await {
				Ok(Ok(())) => Ok(http::Response::builder()
					.status(http::StatusCode::OK)
					.body(http_body_util::Full::new(Bytes::from_static(b"ok")))?),
				Ok(Err(err)) => {
					tracing::error!("opentelemetry flush failed: {err:#}");
					Ok(http::Response::builder()
						.status(http::StatusCode::INTERNAL_SERVER_ERROR)
						.body(http_body_util::Full::new(format!("{err:#}").into()))?)
				}
				Err(err) => {
					tracing::error!("opentelemetry flush spawn failed: {err:#}");
					Ok(http::Response::builder()
						.status(http::StatusCode::INTERNAL_SERVER_ERROR)
						.body(http_body_util::Full::new(format!("{err:#}").into()))?)
				}
			}
		} else {
			Ok(http::Response::builder()
				.status(http::StatusCode::OK)
				.body(http_body_util::Full::new(Bytes::from_static(b"ok")))?)
		}
	} else {
		Ok(http::Response::builder()
			.status(http::StatusCode::NOT_FOUND)
			.body(http_body_util::Full::new(Bytes::from_static(b"not found")))?)
	}
}

#[cfg(feature = "opentelemetry")]
pub mod opentelemetry {
	pub use ::opentelemetry::*;

	#[derive(Debug, thiserror::Error)]
	pub enum OpenTelemetryError {
		#[error("metrics: {0}")]
		Metrics(#[from] opentelemetry_sdk::metrics::MetricError),
		#[error("traces: {0}")]
		Traces(#[from] opentelemetry::trace::TraceError),
		#[error("logs: {0}")]
		Logs(#[from] opentelemetry_sdk::logs::LogError),
	}

	#[derive(Debug, Default, Clone)]
	pub struct OpenTelemetry {
		#[cfg(feature = "opentelemetry-metrics")]
		metrics: Option<opentelemetry_sdk::metrics::SdkMeterProvider>,
		#[cfg(feature = "opentelemetry-traces")]
		traces: Option<opentelemetry_sdk::trace::TracerProvider>,
		#[cfg(feature = "opentelemetry-logs")]
		logs: Option<opentelemetry_sdk::logs::LoggerProvider>,
	}

	impl OpenTelemetry {
		pub fn new() -> Self {
			Self::default()
		}

		pub fn is_enabled(&self) -> bool {
			#[cfg_attr(
				not(any(
					feature = "opentelemetry-metrics",
					feature = "opentelemetry-traces",
					feature = "opentelemetry-logs"
				)),
				allow(unused_mut)
			)]
			let mut enabled = false;
			#[cfg(feature = "opentelemetry-metrics")]
			{
				enabled |= self.metrics.is_some();
			}
			#[cfg(feature = "opentelemetry-traces")]
			{
				enabled |= self.traces.is_some();
			}
			#[cfg(feature = "opentelemetry-logs")]
			{
				enabled |= self.logs.is_some();
			}
			enabled
		}

		#[cfg(feature = "opentelemetry-metrics")]
		pub fn with_metrics(self, metrics: impl Into<Option<opentelemetry_sdk::metrics::SdkMeterProvider>>) -> Self {
			Self {
				metrics: metrics.into(),
				#[cfg(feature = "opentelemetry-traces")]
				traces: self.traces,
				#[cfg(feature = "opentelemetry-logs")]
				logs: self.logs,
			}
		}

		#[cfg(feature = "opentelemetry-traces")]
		pub fn with_traces(self, traces: impl Into<Option<opentelemetry_sdk::trace::TracerProvider>>) -> Self {
			Self {
				traces: traces.into(),
				#[cfg(feature = "opentelemetry-metrics")]
				metrics: self.metrics,
				#[cfg(feature = "opentelemetry-logs")]
				logs: self.logs,
			}
		}

		#[cfg(feature = "opentelemetry-logs")]
		pub fn with_logs(self, logs: impl Into<Option<opentelemetry_sdk::logs::LoggerProvider>>) -> Self {
			Self {
				logs: logs.into(),
				#[cfg(feature = "opentelemetry-traces")]
				traces: self.traces,
				#[cfg(feature = "opentelemetry-metrics")]
				metrics: self.metrics,
			}
		}

		/// Flushes all metrics, traces, and logs, warning; this blocks the
		/// current thread.
		pub fn flush(&self) -> Result<(), OpenTelemetryError> {
			#[cfg(feature = "opentelemetry-metrics")]
			if let Some(metrics) = &self.metrics {
				metrics.force_flush()?;
			}

			#[cfg(feature = "opentelemetry-traces")]
			if let Some(traces) = &self.traces {
				for r in traces.force_flush() {
					r?;
				}
			}

			#[cfg(feature = "opentelemetry-logs")]
			if let Some(logs) = &self.logs {
				for r in logs.force_flush() {
					r?;
				}
			}

			Ok(())
		}

		/// Shuts down all metrics, traces, and logs.
		pub fn shutdown(&self) -> Result<(), OpenTelemetryError> {
			#[cfg(feature = "opentelemetry-metrics")]
			if let Some(metrics) = &self.metrics {
				metrics.shutdown()?;
			}

			#[cfg(feature = "opentelemetry-traces")]
			if let Some(traces) = &self.traces {
				traces.shutdown()?;
			}

			#[cfg(feature = "opentelemetry-logs")]
			if let Some(logs) = &self.logs {
				logs.shutdown()?;
			}

			Ok(())
		}
	}
}
