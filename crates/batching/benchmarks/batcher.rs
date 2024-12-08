use std::future::Future;
use std::marker::PhantomData;
use std::sync::Arc;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use scuffle_batching::batch::BatchResponse;

struct DataloaderImpl<F, P>(F, PhantomData<P>);

impl<F, P> DataloaderImpl<F, P> {
	fn new(f: F) -> Self {
		Self(f, PhantomData)
	}
}

impl<F, Fut> scuffle_batching::BatchExecutor for DataloaderImpl<F, Fut>
where
	F: Fn(Vec<(usize, BatchResponse<usize>)>) -> Fut + Send + Sync,
	Fut: Future<Output = ()> + Send,
	Self: Send + Sync,
{
	type Request = usize;
	type Response = usize;

	async fn execute(&self, keys: Vec<(Self::Request, BatchResponse<Self::Request>)>) {
		(self.0)(keys).await;
	}
}

async fn run_scuffle_batcher_many(
	size: usize,
	loader: impl scuffle_batching::BatchExecutor<Request = usize, Response = usize> + Send + Sync + 'static,
) {
	let batcher = Arc::new(scuffle_batching::Batcher::new(
		loader,
		size / 2,
		100,
		std::time::Duration::from_millis(5),
	));

	let spawn = || {
		let batcher = batcher.clone();
		tokio::spawn(async move { batcher.execute_many(0..size / 4).await })
	};

	futures::future::join_all([spawn(), spawn(), spawn(), spawn()]).await;
}

async fn run_scuffle_batcher_single(
	size: usize,
	loader: impl scuffle_batching::BatchExecutor<Request = usize, Response = usize> + Send + Sync + 'static,
) {
	let batcher = Arc::new(scuffle_batching::Batcher::new(
		loader,
		size / 2,
		100,
		std::time::Duration::from_millis(5),
	));

	let spawn = |i| {
		let batcher = batcher.clone();
		tokio::spawn(async move { batcher.execute(i).await })
	};

	futures::future::join_all((0..size / 4).cycle().take(size).map(spawn)).await;
}

fn delay(c: &mut Criterion) {
	let size: usize = 1000;

	let mut group = c.benchmark_group("batcher - delay");

	let runtime = || tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap();

	group.bench_with_input(BenchmarkId::new("many", size), &size, |b, &s| {
		b.to_async(runtime()).iter(|| async move {
			run_scuffle_batcher_many(
				s,
				DataloaderImpl::new(|keys: Vec<(usize, BatchResponse<usize>)>| async move {
					black_box(tokio::time::sleep(std::time::Duration::from_millis(1))).await;
					for (key, resp) in keys {
						resp.send(black_box(key));
					}
				}),
			)
			.await;
		});
	});

	group.bench_with_input(BenchmarkId::new("single", size), &size, |b, &s| {
		b.to_async(runtime()).iter(|| async move {
			run_scuffle_batcher_single(
				s,
				DataloaderImpl::new(|keys: Vec<(usize, BatchResponse<usize>)>| async move {
					black_box(tokio::time::sleep(std::time::Duration::from_millis(1))).await;
					for (key, resp) in keys {
						resp.send(black_box(key));
					}
				}),
			)
			.await;
		});
	});

	group.finish();
}

criterion_group!(benches, delay);
criterion_main!(benches);
