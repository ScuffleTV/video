use std::sync::Arc;

use anyhow::Context;
use scuffle_signal::{SignalConfig, SignalSvc};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

scuffle_bootstrap::main! {
	Global {
		SignalSvc,
	}
}

impl SignalConfig for Global {
	async fn on_shutdown(self: &Arc<Self>) -> anyhow::Result<()> {
		tracing::info!("on_shutdown");
		Ok(())
	}
}

struct Global;

impl scuffle_bootstrap::global::GlobalWithoutConfig for Global {
	async fn init() -> anyhow::Result<Arc<Self>> {
		tracing_subscriber::registry()
			.with(tracing_subscriber::fmt::layer())
			.try_init()
			.context("set_global_default")?;

		tracing::info!("init");
		Ok(Arc::new(Global))
	}

	async fn on_service_exit(self: &Arc<Self>, name: &'static str, result: anyhow::Result<()>) -> anyhow::Result<()> {
		tracing::info!("on_service_exit: {name}: {:?}", result);
		result
	}

	// Optional method
	async fn on_services_start(self: &Arc<Self>) -> anyhow::Result<()> {
		tracing::info!("on_services_start");
		Ok(())
	}

	async fn on_exit(self: &Arc<Self>, result: anyhow::Result<()>) -> anyhow::Result<()> {
		tracing::info!("on_shutdown_complete: {:?}", result);
		result
	}
}
