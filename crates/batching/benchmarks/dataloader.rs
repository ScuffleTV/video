use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::marker::PhantomData;
use std::sync::Arc;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

struct DataloaderImpl<F, P>(F, PhantomData<P>);

impl<F, P> DataloaderImpl<F, P> {
	fn new(f: F) -> Self {
		Self(f, PhantomData)
	}
}

impl<F, Fut> scuffle_batching::DataLoaderFetcher for DataloaderImpl<F, Fut>
where
	F: Fn(HashSet<usize>) -> Fut + Send + Sync,
	Fut: Future<Output = Option<HashMap<usize, usize>>> + Send,
	Self: Send + Sync,
{
	type Key = usize;
	type Value = usize;

	async fn load(&self, keys: HashSet<Self::Key>) -> Option<HashMap<Self::Key, Self::Value>> {
		(self.0)(keys).await
	}
}

async fn run_scuffle_dataloader_many(
	size: usize,
	loader: impl scuffle_batching::DataLoaderFetcher<Key = usize, Value = usize> + Send + Sync + 'static,
) {
	let dataloader = Arc::new(scuffle_batching::DataLoader::new(
		loader,
		size / 2,
		100,
		std::time::Duration::from_millis(5),
	));

	let spawn = || {
		let dataloader = dataloader.clone();
		tokio::spawn(async move { dataloader.load_many(0..size).await })
	};

	futures::future::join_all([spawn(), spawn(), spawn(), spawn()]).await;
}

async fn run_scuffle_dataloader_single(
	size: usize,
	loader: impl scuffle_batching::DataLoaderFetcher<Key = usize, Value = usize> + Send + Sync + 'static,
) {
	let dataloader = Arc::new(scuffle_batching::DataLoader::new(
		loader,
		size / 2,
		100,
		std::time::Duration::from_millis(5),
	));

	let spawn = |i| {
		let dataloader = dataloader.clone();
		tokio::spawn(async move { dataloader.load(i).await })
	};

	futures::future::join_all((0..size).cycle().take(size * 4).map(spawn)).await;
}

fn delay(c: &mut Criterion) {
	let size: usize = 1000;

	let mut group = c.benchmark_group("dataloader - delay");

	let runtime = || tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap();

	group.bench_with_input(BenchmarkId::new("many", size), &size, |b, &s| {
		b.to_async(runtime()).iter(|| async move {
			run_scuffle_dataloader_many(
				s,
				DataloaderImpl::new(|keys: HashSet<usize>| async move {
					black_box(tokio::time::sleep(std::time::Duration::from_millis(1))).await;
					black_box(Some(keys.into_iter().map(black_box).map(|k| (k, k)).collect()))
				}),
			)
			.await;
		});
	});

	group.bench_with_input(BenchmarkId::new("single", size), &size, |b, &s| {
		b.to_async(runtime()).iter(|| async move {
			run_scuffle_dataloader_single(
				s,
				DataloaderImpl::new(|keys: HashSet<usize>| async move {
					black_box(tokio::time::sleep(std::time::Duration::from_millis(1))).await;
					black_box(Some(keys.into_iter().map(black_box).map(|k| (k, k)).collect()))
				}),
			)
			.await;
		});
	});

	group.finish();
}

criterion_group!(benches, delay);
criterion_main!(benches);
