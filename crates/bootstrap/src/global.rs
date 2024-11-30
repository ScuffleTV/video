use std::sync::Arc;

use crate::config::{ConfigParser, EmptyConfig};

fn default_runtime() -> tokio::runtime::Runtime {
	let worker_threads = std::env::var("TOKIO_WORKER_THREADS")
		.unwrap_or_default()
		.parse::<usize>()
		.ok()
		.or_else(|| std::thread::available_parallelism().ok().map(|p| p.get()));

	let mut builder = if let Some(1) = worker_threads {
		tokio::runtime::Builder::new_current_thread()
	} else {
		tokio::runtime::Builder::new_multi_thread()
	};

	if let Some(worker_threads) = worker_threads {
		builder.worker_threads(worker_threads);
	}

	if let Ok(max_blocking_threads) = std::env::var("TOKIO_MAX_BLOCKING_THREADS")
		.unwrap_or_default()
		.parse::<usize>()
	{
		builder.max_blocking_threads(max_blocking_threads);
	}

	if !std::env::var("TOKIO_DISABLE_TIME")
		.unwrap_or_default()
		.parse::<bool>()
		.ok()
		.unwrap_or(false)
	{
		builder.enable_time();
	}

	if !std::env::var("TOKIO_DISABLE_IO")
		.unwrap_or_default()
		.parse::<bool>()
		.ok()
		.unwrap_or(false)
	{
		builder.enable_io();
	}

	if let Ok(thread_stack_size) = std::env::var("TOKIO_THREAD_STACK_SIZE").unwrap_or_default().parse::<usize>() {
		builder.thread_stack_size(thread_stack_size);
	}

	if let Ok(global_queue_interval) = std::env::var("TOKIO_GLOBAL_QUEUE_INTERVAL")
		.unwrap_or_default()
		.parse::<u32>()
	{
		builder.global_queue_interval(global_queue_interval);
	}

	if let Ok(event_interval) = std::env::var("TOKIO_EVENT_INTERVAL").unwrap_or_default().parse::<u32>() {
		builder.event_interval(event_interval);
	}

	if let Ok(max_io_events_per_tick) = std::env::var("TOKIO_MAX_IO_EVENTS_PER_TICK")
		.unwrap_or_default()
		.parse::<usize>()
	{
		builder.max_io_events_per_tick(max_io_events_per_tick);
	}

	builder.build().expect("runtime build")
}

pub trait Global: Send + Sync + 'static {
	type Config: ConfigParser + Send + 'static;

	/// Builds the tokio runtime for the application.
	#[inline(always)]
	fn tokio_runtime() -> tokio::runtime::Runtime {
		default_runtime()
	}

	/// Called before loading the config.
	#[inline(always)]
	fn pre_init() -> anyhow::Result<()> {
		Ok(())
	}

	/// Initialize the global.
	fn init(config: Self::Config) -> impl std::future::Future<Output = anyhow::Result<Arc<Self>>> + Send;

	/// Called when all services have been started.
	#[inline(always)]
	fn on_services_start(self: &Arc<Self>) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
		std::future::ready(Ok(()))
	}

	/// Called when the shutdown process is complete, right before exiting the
	/// process.
	#[inline(always)]
	fn on_exit(
		self: &Arc<Self>,
		result: anyhow::Result<()>,
	) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
		std::future::ready(result)
	}

	/// Called when a service exits.
	#[inline(always)]
	fn on_service_exit(
		self: &Arc<Self>,
		name: &'static str,
		result: anyhow::Result<()>,
	) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
		let _ = name;
		std::future::ready(result)
	}
}

pub trait GlobalWithoutConfig: Send + Sync + 'static {
	#[inline(always)]
	fn tokio_runtime() -> tokio::runtime::Runtime {
		default_runtime()
	}

	/// Initialize the global.
	fn init() -> impl std::future::Future<Output = anyhow::Result<Arc<Self>>> + Send;

	/// Called when all services have been started.
	#[inline(always)]
	fn on_services_start(self: &Arc<Self>) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
		std::future::ready(Ok(()))
	}

	/// Called when the shutdown process is complete, right before exiting the
	/// process.
	#[inline(always)]
	fn on_exit(
		self: &Arc<Self>,
		result: anyhow::Result<()>,
	) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
		std::future::ready(result)
	}

	/// Called when a service exits.
	#[inline(always)]
	fn on_service_exit(
		self: &Arc<Self>,
		name: &'static str,
		result: anyhow::Result<()>,
	) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
		let _ = name;
		std::future::ready(result)
	}
}

impl<T: GlobalWithoutConfig> Global for T {
	type Config = EmptyConfig;

	#[inline(always)]
	fn tokio_runtime() -> tokio::runtime::Runtime {
		tokio::runtime::Builder::new_multi_thread()
			.enable_all()
			.build()
			.expect("runtime build")
	}

	#[inline(always)]
	fn init(_: Self::Config) -> impl std::future::Future<Output = anyhow::Result<Arc<Self>>> + Send {
		<T as GlobalWithoutConfig>::init()
	}

	#[inline(always)]
	fn on_service_exit(
		self: &Arc<Self>,
		name: &'static str,
		result: anyhow::Result<()>,
	) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
		<T as GlobalWithoutConfig>::on_service_exit(self, name, result)
	}

	#[inline(always)]
	fn on_services_start(self: &Arc<Self>) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
		<T as GlobalWithoutConfig>::on_services_start(self)
	}

	#[inline(always)]
	fn on_exit(
		self: &Arc<Self>,
		result: anyhow::Result<()>,
	) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
		<T as GlobalWithoutConfig>::on_exit(self, result)
	}
}
