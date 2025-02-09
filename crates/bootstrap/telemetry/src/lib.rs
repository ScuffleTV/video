//! A crate used to add telemetry to applications built with the
//! [`scuffle-bootstrap`](../scuffle_bootstrap) crate.
//!
//! Emit metrics using the [`scuffle-metrics`](../scuffle_metrics)
//! crate.
//!
//! ## Feature Flags
//!
//! - `prometheus`: Enable Prometheus support.
//! - `pprof`: Enable pprof support.
//! - `opentelemetry-metrics`: Enable OpenTelemetry metrics support.
//! - `opentelemetry-traces`: Enable OpenTelemetry traces support.
//! - `opentelemetry-logs`: Enable OpenTelemetry logs support.
//!
//! All features are enabled by default.
//!
//! See [`TelemetrySvc`] for more details.
//!
//! ## Example
//!
//! ```rust
//! use std::net::SocketAddr;
//! use std::sync::Arc;
//!
//! use scuffle_bootstrap::global::GlobalWithoutConfig;
//! use scuffle_bootstrap_telemetry::{prometheus_client, opentelemetry, opentelemetry_sdk, TelemetryConfig, TelemetrySvc};
//!
//! struct Global {
//!     prometheus: prometheus_client::registry::Registry,
//!     open_telemetry: opentelemetry::OpenTelemetry,
//! }
//!
//! impl GlobalWithoutConfig for Global {
//!     async fn init() -> anyhow::Result<Arc<Self>> {
//!         // Initialize the Prometheus metrics registry.
//!         let mut prometheus = prometheus_client::registry::Registry::default();
//!         // The exporter converts opentelemetry metrics into the Prometheus format.
//!         let exporter = scuffle_metrics::prometheus::exporter().build();
//!         // Register the exporter as a data source for the Prometheus registry.
//!         prometheus.register_collector(exporter.collector());
//!
//!         // Initialize the OpenTelemetry metrics provider and add the Prometheus exporter as a reader.
//!         let metrics = opentelemetry_sdk::metrics::SdkMeterProvider::builder().with_reader(exporter).build();
//!         opentelemetry::global::set_meter_provider(metrics.clone());
//!
//!         // Initialize the OpenTelemetry configuration instance.
//!         let open_telemetry = opentelemetry::OpenTelemetry::new().with_metrics(metrics);
//!
//!         Ok(Arc::new(Self {
//!             prometheus,
//!             open_telemetry,
//!         }))
//!     }
//! }
//!
//! impl TelemetryConfig for Global {
//!     fn bind_address(&self) -> Option<SocketAddr> {
//!         // Tells the http server to bind to port 8080 on localhost.
//!         Some(SocketAddr::from(([127, 0, 0, 1], 8080)))
//!     }
//!
//!     fn prometheus_metrics_registry(&self) -> Option<&prometheus_client::registry::Registry> {
//!         Some(&self.prometheus)
//!     }
//!
//!     fn opentelemetry(&self) -> Option<&opentelemetry::OpenTelemetry> {
//!         Some(&self.open_telemetry)
//!     }
//! }
//!
//! #[scuffle_metrics::metrics]
//! mod example {
//!     use scuffle_metrics::{CounterU64, MetricEnum};
//!
//!     #[derive(MetricEnum)]
//!     pub enum Kind {
//!         Http,
//!         Grpc,
//!     }
//!
//!     #[metrics(unit = "requests")]
//!     pub fn request(kind: Kind) -> CounterU64;
//! }
//!
//! // Now emit metrics from anywhere in your code using the `example` module.
//! example::request(example::Kind::Http).incr();
//!
//! scuffle_bootstrap::main! {
//!     Global {
//!         TelemetrySvc,
//!     }
//! };
//! ```
//!
//! ## Status
//!
//! This crate is currently under development and is not yet stable, unit tests
//! are not yet fully implemented.
//!
//! Unit tests are not yet fully implemented. Use at your own risk.
//!
//! ## License
//!
//! This project is licensed under the [MIT](./LICENSE.MIT) or
//! [Apache-2.0](./LICENSE.Apache-2.0) license. You can choose between one of
//! them if you use this work.
//!
//! `SPDX-License-Identifier: MIT OR Apache-2.0`
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(all(coverage_nightly, test), feature(coverage_attribute))]

use anyhow::Context;
use bytes::Bytes;
#[cfg(feature = "opentelemetry-logs")]
#[cfg_attr(docsrs, doc(cfg(feature = "opentelemetry-logs")))]
pub use opentelemetry_appender_tracing;
#[cfg(feature = "opentelemetry")]
#[cfg_attr(docsrs, doc(cfg(feature = "opentelemetry")))]
pub use opentelemetry_sdk;
#[cfg(feature = "prometheus")]
#[cfg_attr(docsrs, doc(cfg(feature = "prometheus")))]
pub use prometheus_client;
use scuffle_bootstrap::global::Global;
use scuffle_bootstrap::service::Service;
use scuffle_context::ContextFutExt;
use scuffle_http::backend::HttpServer;
use scuffle_http::body::IncomingBody;
#[cfg(feature = "opentelemetry-traces")]
pub use tracing_opentelemetry;

#[cfg(feature = "opentelemetry")]
#[cfg_attr(docsrs, doc(cfg(feature = "opentelemetry")))]
pub mod opentelemetry;

/// The telemetry service.
///
/// This is supposed to be used with the `scuffle-bootstrap` crate.
///
/// # HTTP Server
///
/// This service provides a http server which will bind to the address provided
/// by the config. (See [`TelemetryConfig`])
///
/// ## Endpoints
///
/// The server provides the following endpoints:
///
/// ### `/health`
///
/// Health check endpoint.
///
/// This endpoint calls the health check function provided by the config and
/// responds with `200 OK` if the health check returns `Ok(())`. If the health
/// check returns an error, the endpoint returns `500 Internal Server Error`
/// along with the error message.
///
/// ### `/metrics`
///
/// Metrics endpoint which can be used by Prometheus to scrape metrics.
///
/// This endpoint is only enabled if the `prometheus` feature flag is enabled
/// and a metrics registry is provided through the config.
///
/// ### `/pprof/cpu`
///
/// pprof cpu endpoint to capture a cpu profile.
///
/// #### Query Parameters
///
/// - `freq`: Sampling frequency in Hz.
/// - `duration`: Duration the profile should be captured for in s.
/// - `ignore`: List of functions to exclude from the profile.
///
/// This endpoint is only enabled if the `pprof` feature flag is enabled.
///
/// ### `/opentelemetry/flush`
///
/// OpenTelemetry flush endpoint.
///
/// This endpoint is only enabled if one of the `opentelemetry` feature flags is
/// enabled and an OpenTelemetry config is provided through the config.
pub struct TelemetrySvc;

/// Implement this trait to configure the telemetry service.
pub trait TelemetryConfig: Global {
    /// Return true if the service is enabled.
    fn enabled(&self) -> bool {
        true
    }

    /// Return the bind address for the http server.
    fn bind_address(&self) -> Option<std::net::SocketAddr> {
        None
    }

    /// Return the http server name.
    fn http_server_name(&self) -> &str {
        "scuffle-bootstrap-telemetry"
    }

    /// Return a health check to determine if the service is healthy.
    ///
    /// Always healthy by default.
    fn health_check(&self) -> impl std::future::Future<Output = Result<(), anyhow::Error>> + Send {
        std::future::ready(Ok(()))
    }

    /// Return a Prometheus metrics registry to scrape metrics from.
    ///
    /// Returning `Some` will enable the `/metrics` http endpoint which can be
    /// used by Prometheus to scrape metrics.
    ///
    /// Disabled (`None`) by default.
    #[cfg(feature = "prometheus")]
    #[cfg_attr(docsrs, doc(cfg(feature = "prometheus")))]
    fn prometheus_metrics_registry(&self) -> Option<&prometheus_client::registry::Registry> {
        None
    }

    /// Pass an OpenTelemetry instance to the service.
    ///
    /// If provided the service will flush and shutdown the OpenTelemetry
    /// instance when it shuts down.
    /// Additionally, the service provides the `/opentelemetry/flush` http
    /// endpoint to manually flush the data.
    #[cfg(feature = "opentelemetry")]
    #[cfg_attr(docsrs, doc(cfg(feature = "opentelemetry")))]
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

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use std::net::SocketAddr;
    use std::sync::Arc;

    use bytes::Bytes;
    use opentelemetry_sdk::logs::LoggerProvider;
    use opentelemetry_sdk::metrics::SdkMeterProvider;
    use opentelemetry_sdk::trace::TracerProvider;
    use scuffle_bootstrap::{GlobalWithoutConfig, Service};

    use crate::{TelemetryConfig, TelemetrySvc};

    async fn request_metrics(addr: SocketAddr) -> reqwest::Result<String> {
        reqwest::get(format!("http://{addr}/metrics"))
            .await
            .unwrap()
            .error_for_status()?
            .text()
            .await
    }

    async fn request_health(addr: SocketAddr) -> String {
        reqwest::get(format!("http://{addr}/health"))
            .await
            .unwrap()
            .error_for_status()
            .expect("health check failed")
            .text()
            .await
            .expect("health check text")
    }

    async fn request_pprof(addr: SocketAddr, freq: &str, duration: &str) -> reqwest::Result<Bytes> {
        reqwest::get(format!("http://{addr}/pprof/cpu?freq={freq}&duration={duration}"))
            .await
            .unwrap()
            .error_for_status()?
            .bytes()
            .await
    }

    async fn flush_opentelemetry(addr: SocketAddr) -> reqwest::Result<reqwest::Response> {
        reqwest::get(format!("http://{addr}/opentelemetry/flush"))
            .await
            .unwrap()
            .error_for_status()
    }

    #[tokio::test]
    async fn telemetry_http_server() {
        struct TestGlobal {
            bind_addr: SocketAddr,
            prometheus: prometheus_client::registry::Registry,
            open_telemetry: crate::opentelemetry::OpenTelemetry,
        }

        impl GlobalWithoutConfig for TestGlobal {
            async fn init() -> anyhow::Result<Arc<Self>> {
                let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
                let bind_addr = listener.local_addr()?;

                let mut prometheus = prometheus_client::registry::Registry::default();

                let exporter = scuffle_metrics::prometheus::exporter().build();
                prometheus.register_collector(exporter.collector());
                let metrics = SdkMeterProvider::builder().with_reader(exporter).build();
                opentelemetry::global::set_meter_provider(metrics.clone());

                let tracer = TracerProvider::default();
                opentelemetry::global::set_tracer_provider(tracer.clone());

                let logger = LoggerProvider::builder().build();

                let open_telemetry = crate::opentelemetry::OpenTelemetry::new()
                    .with_metrics(metrics)
                    .with_traces(tracer)
                    .with_logs(logger);

                Ok(Arc::new(TestGlobal {
                    bind_addr,
                    prometheus,
                    open_telemetry,
                }))
            }
        }

        impl TelemetryConfig for TestGlobal {
            fn bind_address(&self) -> Option<std::net::SocketAddr> {
                Some(self.bind_addr)
            }

            fn prometheus_metrics_registry(&self) -> Option<&prometheus_client::registry::Registry> {
                Some(&self.prometheus)
            }

            fn opentelemetry(&self) -> Option<&crate::opentelemetry::OpenTelemetry> {
                Some(&self.open_telemetry)
            }
        }

        #[scuffle_metrics::metrics]
        mod example {
            use scuffle_metrics::{CounterU64, MetricEnum};

            #[derive(MetricEnum)]
            pub enum Kind {
                Http,
                Grpc,
            }

            #[metrics(unit = "requests")]
            pub fn request(kind: Kind) -> CounterU64;
        }

        let global = <TestGlobal as GlobalWithoutConfig>::init().await.unwrap();

        let bind_addr = global.bind_addr;

        assert!(TelemetrySvc.enabled(&global).await.unwrap());

        let task_handle = tokio::spawn(TelemetrySvc.run(global, scuffle_context::Context::global()));

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let health = request_health(bind_addr).await;
        assert_eq!(health, "ok");

        let metrics = request_metrics(bind_addr).await.expect("metrics failed");
        assert!(metrics.starts_with("# HELP target Information about the target\n"));
        assert!(metrics.contains("# TYPE target info\n"));
        assert!(metrics.contains("service_name=\"unknown_service\""));
        assert!(metrics.contains("telemetry_sdk_language=\"rust\""));
        assert!(metrics.contains("telemetry_sdk_name=\"opentelemetry\""));
        assert!(metrics.ends_with("# EOF\n"));

        example::request(example::Kind::Http).incr();

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let metrics = request_metrics(bind_addr).await.expect("metrics failed");
        assert!(metrics.contains("# UNIT example_request_requests requests\n"));
        assert!(metrics.contains("example_request_requests_total{"));
        assert!(metrics.contains(format!("otel_scope_name=\"{}\"", env!("CARGO_PKG_NAME")).as_str()));
        assert!(metrics.contains(format!("otel_scope_version=\"{}\"", env!("CARGO_PKG_VERSION")).as_str()));
        assert!(metrics.contains("kind=\"Http\""));
        assert!(metrics.contains("} 1\n"));
        assert!(metrics.ends_with("# EOF\n"));

        example::request(example::Kind::Http).incr();

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let metrics = request_metrics(bind_addr).await.expect("metrics failed");
        assert!(metrics.contains("# UNIT example_request_requests requests\n"));
        assert!(metrics.contains("example_request_requests_total{"));
        assert!(metrics.contains(format!("otel_scope_name=\"{}\"", env!("CARGO_PKG_NAME")).as_str()));
        assert!(metrics.contains(format!("otel_scope_version=\"{}\"", env!("CARGO_PKG_VERSION")).as_str()));
        assert!(metrics.contains("kind=\"Http\""));
        assert!(metrics.contains("} 2\n"));
        assert!(metrics.ends_with("# EOF\n"));

        let timer = std::time::Instant::now();
        assert!(!request_pprof(bind_addr, "100", "2").await.expect("pprof failed").is_empty());
        assert!(timer.elapsed() > std::time::Duration::from_secs(2));

        let res = request_pprof(bind_addr, "invalid", "2").await.expect_err("error expected");
        assert!(res.is_status());
        assert_eq!(res.status(), Some(reqwest::StatusCode::BAD_REQUEST));

        let res = request_pprof(bind_addr, "100", "invalid").await.expect_err("error expected");
        assert!(res.is_status());
        assert_eq!(res.status(), Some(reqwest::StatusCode::BAD_REQUEST));

        assert!(flush_opentelemetry(bind_addr).await.is_ok());

        // Not found
        let res = reqwest::get(format!("http://{bind_addr}/not_found")).await.unwrap();
        assert_eq!(res.status(), reqwest::StatusCode::NOT_FOUND);

        scuffle_context::Handler::global().shutdown().await;

        task_handle.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn empty_telemetry_http_server() {
        struct TestGlobal {
            bind_addr: SocketAddr,
        }

        impl GlobalWithoutConfig for TestGlobal {
            async fn init() -> anyhow::Result<Arc<Self>> {
                let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
                let bind_addr = listener.local_addr()?;

                Ok(Arc::new(TestGlobal { bind_addr }))
            }
        }

        impl TelemetryConfig for TestGlobal {
            fn bind_address(&self) -> Option<std::net::SocketAddr> {
                Some(self.bind_addr)
            }
        }

        let global = <TestGlobal as GlobalWithoutConfig>::init().await.unwrap();

        let bind_addr = global.bind_addr;

        assert!(TelemetrySvc.enabled(&global).await.unwrap());

        let task_handle = tokio::spawn(TelemetrySvc.run(global, scuffle_context::Context::global()));
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let health = request_health(bind_addr).await;
        assert_eq!(health, "ok");

        let res = request_metrics(bind_addr).await.expect_err("error expected");
        assert!(res.is_status());
        assert_eq!(res.status(), Some(reqwest::StatusCode::NOT_FOUND));

        let timer = std::time::Instant::now();
        assert!(!request_pprof(bind_addr, "100", "2").await.expect("pprof failed").is_empty());
        assert!(timer.elapsed() > std::time::Duration::from_secs(2));

        let err = flush_opentelemetry(bind_addr).await.expect_err("error expected");
        assert!(err.is_status());
        assert_eq!(err.status(), Some(reqwest::StatusCode::NOT_FOUND));

        scuffle_context::Handler::global().shutdown().await;

        task_handle.await.unwrap().unwrap();
    }
}
