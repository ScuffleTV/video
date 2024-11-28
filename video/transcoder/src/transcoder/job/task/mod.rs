use futures_util::Future;

pub mod generic;
pub mod recording;
pub mod rendition;
pub mod track_parser;

async fn retry_task<F: Future<Output = anyhow::Result<()>> + Send>(
	func: impl Fn() -> F,
	count: usize,
) -> anyhow::Result<()> {
	let mut count = count;
	loop {
		let Err(err) = func().await else {
			return Ok(());
		};
		if count == 0 {
			return Err(err);
		}

		count -= 1;
		tracing::warn!("failed to complete task: {:#}", err);
		tokio::time::sleep(std::time::Duration::from_secs(1)).await;
	}
}
