use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

/// A trait for fetching data in batches
pub trait DataLoaderFetcher {
	/// The incoming key type
	type Key: Clone + Eq + std::hash::Hash + Send + Sync;
	/// The outgoing value type
	type Value: Clone + Send + Sync;

	/// Load a batch of keys
	fn load(&self, keys: HashSet<Self::Key>) -> impl Future<Output = Option<HashMap<Self::Key, Self::Value>>> + Send;
}

/// A builder for a [`DataLoader`]
#[derive(Clone, Copy, Debug)]
#[must_use = "builders must be used to create a dataloader"]
pub struct DataLoaderBuilder {
	batch_size: usize,
	concurrency: usize,
	delay: std::time::Duration,
}

impl Default for DataLoaderBuilder {
	fn default() -> Self {
		Self::new()
	}
}

impl DataLoaderBuilder {
	/// Create a new builder
	pub fn new() -> Self {
		Self {
			batch_size: 1000,
			concurrency: 50,
			delay: std::time::Duration::from_millis(5),
		}
	}

	/// Set the batch size
	pub fn batch_size(mut self, batch_size: usize) -> Self {
		self.batch_size = batch_size;
		self
	}

	/// Set the delay
	pub fn delay(mut self, delay: std::time::Duration) -> Self {
		self.delay = delay;
		self
	}

	/// Set the concurrency
	pub fn concurrency(mut self, concurrency: usize) -> Self {
		self.concurrency = concurrency;
		self
	}

	/// Build the dataloader
	pub fn build<E>(self, executor: E) -> DataLoader<E>
	where
		E: DataLoaderFetcher + Send + Sync + 'static,
	{
		DataLoader::new(executor, self.batch_size, self.concurrency, self.delay)
	}
}

/// A dataloader used to batch requests to a [`DataLoaderFetcher`]
#[must_use = "dataloaders must be used to load data"]
pub struct DataLoader<E>
where
	E: DataLoaderFetcher + Send + Sync + 'static,
{
	_auto_spawn: tokio::task::JoinHandle<()>,
	executor: Arc<E>,
	semaphore: Arc<tokio::sync::Semaphore>,
	current_batch: Arc<tokio::sync::Mutex<Option<Batch<E>>>>,
	batch_size: usize,
	batch_id: AtomicU64,
}

impl<E> DataLoader<E>
where
	E: DataLoaderFetcher + Send + Sync + 'static,
{
	/// Create a new dataloader
	pub fn new(executor: E, batch_size: usize, concurrency: usize, delay: std::time::Duration) -> Self {
		let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency.max(1)));
		let current_batch = Arc::new(tokio::sync::Mutex::new(None));
		let executor = Arc::new(executor);

		let join_handle = tokio::spawn(batch_loop(executor.clone(), current_batch.clone(), delay));

		Self {
			executor,
			_auto_spawn: join_handle,
			semaphore,
			current_batch,
			batch_size: batch_size.max(1),
			batch_id: AtomicU64::new(0),
		}
	}

	/// Create a builder for a [`DataLoader`]
	pub fn builder() -> DataLoaderBuilder {
		DataLoaderBuilder::new()
	}

	/// Load a single key
	/// Can return an error if the underlying [`DataLoaderFetcher`] returns an
	/// error
	///
	/// Returns `None` if the key is not found
	pub async fn load(&self, items: E::Key) -> Result<Option<E::Value>, ()> {
		Ok(self.load_many(std::iter::once(items)).await?.into_values().next())
	}

	/// Load many keys
	/// Can return an error if the underlying [`DataLoaderFetcher`] returns an
	/// error
	///
	/// Returns a map of keys to values which may be incomplete if any of the
	/// keys were not found
	pub async fn load_many<I>(&self, items: I) -> Result<HashMap<E::Key, E::Value>, ()>
	where
		I: IntoIterator<Item = E::Key> + Send,
	{
		struct BatchWaiting<K, V> {
			id: u64,
			keys: HashSet<K>,
			result: Arc<BatchResult<K, V>>,
		}

		let mut waiters = Vec::<BatchWaiting<E::Key, E::Value>>::new();

		let mut count = 0;

		{
			let mut batch = self.current_batch.lock().await;

			for item in items {
				if batch.is_none() {
					batch.replace(Batch::new(
						self.batch_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
						self.semaphore.clone(),
					));
				}

				let batch_mut = batch.as_mut().unwrap();
				batch_mut.items.insert(item.clone());

				if waiters.is_empty() || waiters.last().unwrap().id != batch_mut.id {
					waiters.push(BatchWaiting {
						id: batch_mut.id,
						keys: HashSet::new(),
						result: batch_mut.result.clone(),
					});
				}

				let waiting = waiters.last_mut().unwrap();
				waiting.keys.insert(item);

				count += 1;

				if batch_mut.items.len() >= self.batch_size {
					tokio::spawn(batch.take().unwrap().spawn(self.executor.clone()));
				}
			}
		}

		let mut results = HashMap::with_capacity(count);
		for waiting in waiters {
			let result = waiting.result.wait().await?;
			results.extend(waiting.keys.into_iter().filter_map(|key| {
				let value = result.get(&key)?.clone();
				Some((key, value))
			}));
		}

		Ok(results)
	}
}

async fn batch_loop<E>(
	executor: Arc<E>,
	current_batch: Arc<tokio::sync::Mutex<Option<Batch<E>>>>,
	delay: std::time::Duration,
) where
	E: DataLoaderFetcher + Send + Sync + 'static,
{
	let mut pending_id = None;
	loop {
		tokio::time::sleep(delay).await;

		let mut batch = current_batch.lock().await;
		let Some(batch_id) = batch.as_ref().map(|b| b.id) else {
			pending_id = None;
			continue;
		};

		if pending_id != Some(batch_id) || batch.as_ref().unwrap().items.is_empty() {
			pending_id = Some(batch_id);
			continue;
		}

		tokio::spawn(batch.take().unwrap().spawn(executor.clone()));
	}
}

struct BatchResult<K, V> {
	values: tokio::sync::OnceCell<Option<HashMap<K, V>>>,
	token: tokio_util::sync::CancellationToken,
}

impl<K, V> BatchResult<K, V> {
	fn new() -> Self {
		Self {
			values: tokio::sync::OnceCell::new(),
			token: tokio_util::sync::CancellationToken::new(),
		}
	}

	async fn wait(&self) -> Result<&HashMap<K, V>, ()> {
		if !self.token.is_cancelled() {
			self.token.cancelled().await;
		}

		self.values.get().ok_or(())?.as_ref().ok_or(())
	}
}

struct Batch<E>
where
	E: DataLoaderFetcher + Send + Sync + 'static,
{
	id: u64,
	items: HashSet<E::Key>,
	result: Arc<BatchResult<E::Key, E::Value>>,
	semaphore: Arc<tokio::sync::Semaphore>,
}

impl<E> Batch<E>
where
	E: DataLoaderFetcher + Send + Sync + 'static,
{
	fn new(id: u64, semaphore: Arc<tokio::sync::Semaphore>) -> Self {
		Self {
			id,
			items: HashSet::new(),
			result: Arc::new(BatchResult::new()),
			semaphore,
		}
	}

	async fn spawn(self, executor: Arc<E>) {
		let _drop_guard = self.result.token.clone().drop_guard();
		let _ticket = self.semaphore.acquire_owned().await.unwrap();
		let result = executor.load(self.items).await;
		match self.result.values.set(result) {
			Ok(()) => {}
			Err(_) => unreachable!(
				"batch result already set, this is a bug please report it https://github.com/scufflecloud/scuffle/issues"
			),
		}
	}
}
