use std::sync::Arc;

use scuffle_bootstrap::global::Global;
use scuffle_bootstrap::service::Service;
use scuffle_context::ContextFutExt;

#[derive(Default, Debug, Clone, Copy)]
pub struct SignalSvc;

pub trait SignalConfig: Global {
	fn signals(&self) -> Vec<tokio::signal::unix::SignalKind> {
		vec![
			tokio::signal::unix::SignalKind::terminate(),
			tokio::signal::unix::SignalKind::interrupt(),
		]
	}

	fn timeout(&self) -> Option<std::time::Duration> {
		Some(std::time::Duration::from_secs(30))
	}

	fn on_shutdown(self: &Arc<Self>) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
		std::future::ready(Ok(()))
	}

	fn on_force_shutdown(
		&self,
		signal: Option<tokio::signal::unix::SignalKind>,
	) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
		let err = if let Some(signal) = signal {
			anyhow::anyhow!("received signal, shutting down immediately: {:?}", signal)
		} else {
			anyhow::anyhow!("timeout reached, shutting down immediately")
		};

		std::future::ready(Err(err))
	}
}

impl<Global: SignalConfig> Service<Global> for SignalSvc {
	fn enabled(&self, global: &Arc<Global>) -> impl std::future::Future<Output = anyhow::Result<bool>> + Send {
		std::future::ready(Ok(!global.signals().is_empty()))
	}

	async fn run(self, global: Arc<Global>, ctx: scuffle_context::Context) -> anyhow::Result<()> {
		let timeout = global.timeout();

		let signals = global.signals();
		let mut handler = crate::SignalHandler::with_signals(signals);

		// Wait for a signal, or for the context to be done.
		handler.recv().with_context(&ctx).await;
		global.on_shutdown().await?;
		drop(ctx);

		tokio::select! {
			signal = handler.recv() => {
				global.on_force_shutdown(Some(signal)).await?;
			},
			_ = scuffle_context::Handler::global().shutdown() => {}
			Some(()) = async {
				if let Some(timeout) = timeout {
					tokio::time::sleep(timeout).await;
					Some(())
				} else {
					None
				}
			} => {
				global.on_force_shutdown(None).await?;
			},
		};

		Ok(())
	}
}
