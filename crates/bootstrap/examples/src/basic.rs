use std::sync::Arc;

use scuffle_bootstrap::prelude::*;
use scuffle_bootstrap::service::Service;
use scuffle_signal::{SignalConfig, SignalSvc};

scuffle_bootstrap::main! {
	Global {
		SignalSvc,
		MySvc,
		MySvc2::new(1),
	}
}

struct MySvc;

impl Service<Global> for MySvc {
	async fn run(self, _: Arc<Global>, ctx: scuffle_context::Context) -> anyhow::Result<()> {
		println!("running");
		ctx.done().await;
		// Graceful shutdown!
		println!("shutdown requested, exiting");
		anyhow::bail!("shutdown requested")
	}
}

struct MySvc2(u8);

impl MySvc2 {
	fn new(v: u8) -> Self {
		Self(v)
	}
}

impl Service<Global> for MySvc2 {
	async fn run(self, _: Arc<Global>, ctx: scuffle_context::Context) -> anyhow::Result<()> {
		println!("running: {:?}", self.0);
		ctx.done().await;
		// Graceful shutdown!
		println!("shutdown requested, exiting");
		tokio::time::sleep(std::time::Duration::from_secs(10)).await;
		anyhow::bail!("shutdown requested")
	}
}

// Optional methods
impl SignalConfig for Global {}

struct Global;

impl scuffle_bootstrap::global::GlobalWithoutConfig for Global {
	async fn init() -> anyhow::Result<Arc<Self>> {
		Ok(Arc::new(Global))
	}
}
