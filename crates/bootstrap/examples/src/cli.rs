use std::sync::Arc;

use scuffle_bootstrap::prelude::*;
use scuffle_bootstrap::service::Service;
use scuffle_signal::{SignalConfig, SignalSvc};

scuffle_bootstrap::main! {
	Global {
		SignalSvc,
		MySvc,
		svc_fn,
	}
}

impl SignalConfig for Global {}

struct MySvc;

impl Default for MySvc {
	fn default() -> Self {
		Self
	}
}

async fn svc_fn(global: Arc<Global>, ctx: scuffle_context::Context) -> anyhow::Result<()> {
	println!("running");
	let _ = (global, ctx);
	Ok(())
}

impl Service<Global> for MySvc {
	async fn enabled(&self, global: &Arc<Global>) -> anyhow::Result<bool> {
		let _ = global;
		Ok(true)
	}

	async fn run(self, global: Arc<Global>, ctx: scuffle_context::Context) -> anyhow::Result<()> {
		println!("running: {:?}", global.config);
		ctx.done().await;
		Ok(())
	}
}

struct Global {
	pub config: Config,
}

#[derive(serde_derive::Deserialize, Debug, smart_default::SmartDefault)]
#[serde(default)]
struct Config {
	#[default = "foo"]
	#[allow(dead_code)]
	pub arg: String,
}

scuffle_settings::bootstrap!(Config);

impl scuffle_bootstrap::global::Global for Global {
	type Config = Config;

	async fn init(config: Self::Config) -> anyhow::Result<Arc<Self>> {
		Ok(Arc::new(Global { config }))
	}

	async fn on_service_exit(self: &Arc<Self>, name: &'static str, result: anyhow::Result<()>) -> anyhow::Result<()> {
		println!("service exited: {}", name);
		result
	}
}
