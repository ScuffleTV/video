use std::sync::Arc;

use anyhow::Context;
use scuffle_bootstrap::service::Service;
use scuffle_context::ContextFutExt;

use crate::database::Job;
use crate::global::Global;

pub mod process;

pub use self::process::JobError;

pub struct WorkerSvc;

impl Service<Global> for WorkerSvc {
	async fn enabled(&self, global: &Arc<Global>) -> anyhow::Result<bool> {
		Ok(global.config().worker.enabled)
	}

	async fn run(self, global: Arc<Global>, ctx: scuffle_context::Context) -> anyhow::Result<()> {
		let config = global.config();

		let mut concurrency = config.worker.concurrency;

		if concurrency == 0 {
			concurrency = std::thread::available_parallelism().map_or(1, |p| p.get());
		}

		let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));

		tracing::info!("worker started with {} threads", concurrency);

		let mut error_count = 0;
		let (sub_ctx, handle) = ctx.new_child();
		drop(sub_ctx);

		async {
			loop {
				let permit = semaphore.clone().acquire_owned().await.expect("semaphore permit");

				let job = match Job::fetch(&global).await {
					Ok(Some(job)) => {
						tracing::debug!("fetched job");
						job
					}
					Ok(None) => {
						tracing::debug!("no jobs found");
						tokio::time::sleep(config.worker.polling_interval).await;
						continue;
					}
					Err(err) => {
						tracing::error!("failed to fetch job: {err}");
						error_count += 1;
						if error_count >= config.worker.error_threshold {
							return Err::<(), _>(err).context("reached error threshold");
						}

						tokio::time::sleep(config.worker.error_delay).await;

						continue;
					}
				};

				error_count = 0;
				tokio::spawn(self::process::spawn(job, global.clone(), handle.context(), permit));
			}
		}
		.with_context(&ctx)
		.await
		.transpose()?;

		tracing::info!("worker shutdown");
		handle.shutdown().await;

		Ok(())
	}
}
