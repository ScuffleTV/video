use std::future::Future;
use std::sync::Arc;

use tokio::sync::oneshot;

/// A response to a batch request
pub struct BatchResponse<Resp> {
	send: oneshot::Sender<Resp>,
}

impl<Resp> BatchResponse<Resp> {
	/// Create a new batch response
	#[must_use]
	pub fn new(send: oneshot::Sender<Resp>) -> Self {
		Self { send }
	}

	/// Send a response back to the requester
	#[inline(always)]
	pub fn send(self, response: Resp) {
		let _ = self.send.send(response);
	}

	/// Send a successful response back to the requester
	#[inline(always)]
	pub fn send_ok<O, E>(self, response: O)
	where
		Resp: From<Result<O, E>>,
	{
		self.send(Ok(response).into())
	}

	/// Send an error response back to the requestor
	#[inline(always)]
	pub fn send_err<O, E>(self, error: E)
	where
		Resp: From<Result<O, E>>,
	{
		self.send(Err(error).into())
	}

	/// Send a `None` response back to the requestor
	#[inline(always)]
	pub fn send_none<T>(self)
	where
		Resp: From<Option<T>>,
	{
		self.send(None.into())
	}

	/// Send a value response back to the requestor
	#[inline(always)]
	pub fn send_some<T>(self, value: T)
	where
		Resp: From<Option<T>>,
	{
		self.send(Some(value).into())
	}
}

/// A trait for executing batches
pub trait BatchExecutor {
	/// The incoming request type
	type Request: Send + 'static;
	/// The outgoing response type
	type Response: Send + Sync + 'static;

	/// Execute a batch of requests
	/// You must call `send` on the `BatchResponse` to send the response back to
	/// the client
	fn execute(&self, requests: Vec<(Self::Request, BatchResponse<Self::Response>)>) -> impl Future<Output = ()> + Send;
}

/// A builder for a [`Batcher`]
#[derive(Clone, Copy, Debug)]
#[must_use = "builders must be used to create a batcher"]
pub struct BatcherBuilder<E> {
	batch_size: usize,
	concurrency: usize,
	delay: std::time::Duration,
	_marker: std::marker::PhantomData<E>,
}

impl<E> Default for BatcherBuilder<E> {
	fn default() -> Self {
		Self::new()
	}
}

impl<E> BatcherBuilder<E> {
	/// Create a new builder
	pub const fn new() -> Self {
		Self {
			batch_size: 1000,
			concurrency: 50,
			delay: std::time::Duration::from_millis(5),
			_marker: std::marker::PhantomData,
		}
	}

	/// Set the batch size
	#[inline]
	pub const fn batch_size(mut self, batch_size: usize) -> Self {
		self.with_batch_size(batch_size);
		self
	}

	/// Set the delay
	#[inline]
	pub const fn delay(mut self, delay: std::time::Duration) -> Self {
		self.with_delay(delay);
		self
	}

	/// Set the concurrency to 1
	#[inline]
	pub const fn concurrency(mut self, concurrency: usize) -> Self {
		self.with_concurrency(concurrency);
		self
	}

	/// Set the concurrency
	#[inline]
	pub const fn with_concurrency(&mut self, concurrency: usize) -> &mut Self {
		self.concurrency = concurrency;
		self
	}

	/// Set the batch size
	#[inline]
	pub const fn with_batch_size(&mut self, batch_size: usize) -> &mut Self {
		self.batch_size = batch_size;
		self
	}

	/// Set the delay
	#[inline]
	pub const fn with_delay(&mut self, delay: std::time::Duration) -> &mut Self {
		self.delay = delay;
		self
	}

	/// Build the batcher
	#[inline]
	pub fn build(self, executor: E) -> Batcher<E>
	where
		E: BatchExecutor + Send + Sync + 'static,
	{
		Batcher::new(executor, self.batch_size, self.concurrency, self.delay)
	}
}

/// A batcher used to batch requests to a [`BatchExecutor`]
#[must_use = "batchers must be used to execute batches"]
pub struct Batcher<E>
where
	E: BatchExecutor + Send + Sync + 'static,
{
	_auto_spawn: tokio::task::JoinHandle<()>,
	executor: Arc<E>,
	semaphore: Arc<tokio::sync::Semaphore>,
	current_batch: Arc<tokio::sync::Mutex<Option<Batch<E>>>>,
	batch_size: usize,
}

struct Batch<E>
where
	E: BatchExecutor + Send + Sync + 'static,
{
	items: Vec<(E::Request, BatchResponse<E::Response>)>,
	semaphore: Arc<tokio::sync::Semaphore>,
	created_at: std::time::Instant,
}

impl<E> Batcher<E>
where
	E: BatchExecutor + Send + Sync + 'static,
{
	/// Create a new batcher
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
		}
	}

	/// Create a builder for a [`Batcher`]
	pub const fn builder() -> BatcherBuilder<E> {
		BatcherBuilder::new()
	}

	/// Execute a single request
	pub async fn execute(&self, items: E::Request) -> Option<E::Response> {
		self.execute_many(std::iter::once(items)).await.pop()?
	}

	/// Execute many requests
	pub async fn execute_many<I>(&self, items: I) -> Vec<Option<E::Response>>
	where
		I: IntoIterator<Item = E::Request>,
	{
		let mut responses = Vec::new();

		{
			let mut batch = self.current_batch.lock().await;

			for item in items {
				if batch.is_none() {
					batch.replace(Batch::new(self.semaphore.clone()));
				}

				let batch_mut = batch.as_mut().unwrap();
				let (tx, rx) = oneshot::channel();
				batch_mut.items.push((item, BatchResponse::new(tx)));
				responses.push(rx);

				if batch_mut.items.len() >= self.batch_size {
					tokio::spawn(batch.take().unwrap().spawn(self.executor.clone()));
				}
			}
		}

		let mut results = Vec::with_capacity(responses.len());
		for response in responses {
			results.push(response.await.ok());
		}

		results
	}
}

async fn batch_loop<E>(
	executor: Arc<E>,
	current_batch: Arc<tokio::sync::Mutex<Option<Batch<E>>>>,
	delay: std::time::Duration,
) where
	E: BatchExecutor + Send + Sync + 'static,
{
	let mut delay_delta = delay;
	loop {
		tokio::time::sleep(delay_delta).await;

		let mut batch = current_batch.lock().await;
		let Some(created_at) = batch.as_ref().map(|b| b.created_at) else {
			delay_delta = delay;
			continue;
		};

		let remaining = delay.saturating_sub(created_at.elapsed());
		if remaining == std::time::Duration::ZERO {
			tokio::spawn(batch.take().unwrap().spawn(executor.clone()));
			delay_delta = delay;
		} else {
			delay_delta = remaining;
		}
	}
}

impl<E> Batch<E>
where
	E: BatchExecutor + Send + Sync + 'static,
{
	fn new(semaphore: Arc<tokio::sync::Semaphore>) -> Self {
		Self {
			created_at: std::time::Instant::now(),
			items: Vec::new(),
			semaphore,
		}
	}

	async fn spawn(self, executor: Arc<E>) {
		let _ticket = self.semaphore.acquire_owned().await;
		executor.execute(self.items).await;
	}
}

#[cfg_attr(all(coverage_nightly, test), coverage(off))]
#[cfg(test)]
mod tests {
	use std::collections::HashMap;
	use std::sync::atomic::AtomicUsize;

	use super::*;

	struct TestExecutor<K, V> {
		values: HashMap<K, V>,
		delay: std::time::Duration,
		requests: Arc<AtomicUsize>,
		capacity: usize,
	}

	impl<K, V> BatchExecutor for TestExecutor<K, V>
	where
		K: Clone + Eq + std::hash::Hash + Send + Sync + 'static,
		V: Clone + Send + Sync + 'static,
	{
		type Request = K;
		type Response = V;

		async fn execute(&self, requests: Vec<(Self::Request, BatchResponse<Self::Response>)>) {
			tokio::time::sleep(self.delay).await;

			assert!(requests.len() <= self.capacity);

			self.requests.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
			for (request, response) in requests {
				if let Some(value) = self.values.get(&request) {
					response.send(value.clone());
				}
			}
		}
	}

	#[tokio::test]
	async fn basic() {
		let requests = Arc::new(AtomicUsize::new(0));

		let fetcher = TestExecutor {
			values: HashMap::from_iter(vec![("a", 1), ("b", 2), ("c", 3)]),
			delay: std::time::Duration::from_millis(5),
			requests: requests.clone(),
			capacity: 2,
		};

		let loader = Batcher::builder().batch_size(2).concurrency(1).build(fetcher);

		let start = std::time::Instant::now();
		let a = loader.execute("a").await;
		assert_eq!(a, Some(1));
		assert!(start.elapsed() < std::time::Duration::from_millis(15));
		assert_eq!(requests.load(std::sync::atomic::Ordering::Relaxed), 1);

		let start = std::time::Instant::now();
		let b = loader.execute("b").await;
		assert_eq!(b, Some(2));
		assert!(start.elapsed() < std::time::Duration::from_millis(15));
		assert_eq!(requests.load(std::sync::atomic::Ordering::Relaxed), 2);
		let start = std::time::Instant::now();
		let c = loader.execute("c").await;
		assert_eq!(c, Some(3));
		assert!(start.elapsed() < std::time::Duration::from_millis(15));
		assert_eq!(requests.load(std::sync::atomic::Ordering::Relaxed), 3);

		let start = std::time::Instant::now();
		let ab = loader.execute_many(vec!["a", "b"]).await;
		assert_eq!(ab, vec![Some(1), Some(2)]);
		assert!(start.elapsed() < std::time::Duration::from_millis(15));
		assert_eq!(requests.load(std::sync::atomic::Ordering::Relaxed), 4);

		let start = std::time::Instant::now();
		let unknown = loader.execute("unknown").await;
		assert_eq!(unknown, None);
		assert!(start.elapsed() < std::time::Duration::from_millis(15));
		assert_eq!(requests.load(std::sync::atomic::Ordering::Relaxed), 5);
	}

	#[tokio::test]
	async fn concurrency_high() {
		let requests = Arc::new(AtomicUsize::new(0));

		let fetcher = TestExecutor {
			values: HashMap::from_iter(vec![("a", 1), ("b", 2), ("c", 3)]),
			delay: std::time::Duration::from_millis(5),
			requests: requests.clone(),
			capacity: 2,
		};

		let loader = Batcher::builder().batch_size(2).concurrency(10).build(fetcher);

		let start = std::time::Instant::now();
		let ab = loader
			.execute_many(vec!["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"])
			.await;
		assert_eq!(ab, vec![Some(1), Some(2), Some(3), None, None, None, None, None, None, None]);
		assert!(start.elapsed() < std::time::Duration::from_millis(15));
		assert_eq!(requests.load(std::sync::atomic::Ordering::Relaxed), 5);
	}

	#[tokio::test]
	async fn delay_low() {
		let requests = Arc::new(AtomicUsize::new(0));

		let fetcher = TestExecutor {
			values: HashMap::from_iter(vec![("a", 1), ("b", 2), ("c", 3)]),
			delay: std::time::Duration::from_millis(5),
			requests: requests.clone(),
			capacity: 2,
		};

		let loader = Batcher::builder()
			.batch_size(2)
			.concurrency(1)
			.delay(std::time::Duration::from_millis(10))
			.build(fetcher);

		let start = std::time::Instant::now();
		let ab = loader
			.execute_many(vec!["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"])
			.await;
		assert_eq!(ab, vec![Some(1), Some(2), Some(3), None, None, None, None, None, None, None]);
		assert!(start.elapsed() < std::time::Duration::from_millis(35));
		assert!(start.elapsed() >= std::time::Duration::from_millis(25));
		assert_eq!(requests.load(std::sync::atomic::Ordering::Relaxed), 5);
	}

	#[tokio::test]
	async fn batch_size() {
		let requests = Arc::new(AtomicUsize::new(0));

		let fetcher = TestExecutor {
			values: HashMap::from_iter(vec![("a", 1), ("b", 2), ("c", 3)]),
			delay: std::time::Duration::from_millis(5),
			requests: requests.clone(),
			capacity: 100,
		};

		let loader = BatcherBuilder::default()
			.batch_size(100)
			.concurrency(1)
			.delay(std::time::Duration::from_millis(10))
			.build(fetcher);

		let start = std::time::Instant::now();
		let ab = loader
			.execute_many(vec!["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"])
			.await;
		assert_eq!(ab, vec![Some(1), Some(2), Some(3), None, None, None, None, None, None, None]);
		assert!(start.elapsed() >= std::time::Duration::from_millis(10));
		assert_eq!(requests.load(std::sync::atomic::Ordering::Relaxed), 1);
	}

	#[tokio::test]
	async fn high_concurrency() {
		let requests = Arc::new(AtomicUsize::new(0));

		let fetcher = TestExecutor {
			values: HashMap::from_iter((0..1134).map(|i| (i, i * 2 + 5))),
			delay: std::time::Duration::from_millis(5),
			requests: requests.clone(),
			capacity: 100,
		};

		let loader = BatcherBuilder::default()
			.batch_size(100)
			.concurrency(10)
			.delay(std::time::Duration::from_millis(10))
			.build(fetcher);

		let start = std::time::Instant::now();
		let ab = loader.execute_many(0..1134).await;
		assert_eq!(ab, (0..1134).map(|i| Some(i * 2 + 5)).collect::<Vec<_>>());
		assert!(start.elapsed() >= std::time::Duration::from_millis(15));
		assert!(start.elapsed() < std::time::Duration::from_millis(25));
		assert_eq!(requests.load(std::sync::atomic::Ordering::Relaxed), 1134 / 100 + 1);
	}

	#[tokio::test]
	async fn delayed_start() {
		let requests = Arc::new(AtomicUsize::new(0));

		let fetcher = TestExecutor {
			values: HashMap::from_iter(vec![("a", 1), ("b", 2), ("c", 3)]),
			delay: std::time::Duration::from_millis(5),
			requests: requests.clone(),
			capacity: 2,
		};

		let loader = BatcherBuilder::default()
			.batch_size(2)
			.concurrency(100)
			.delay(std::time::Duration::from_millis(10))
			.build(fetcher);

		tokio::time::sleep(std::time::Duration::from_millis(20)).await;

		let start = std::time::Instant::now();
		let ab = loader
			.execute_many(vec!["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"])
			.await;
		assert_eq!(ab, vec![Some(1), Some(2), Some(3), None, None, None, None, None, None, None]);
		assert!(start.elapsed() >= std::time::Duration::from_millis(5));
		assert!(start.elapsed() < std::time::Duration::from_millis(25));
	}

	#[tokio::test]
	async fn delayed_start_single() {
		let requests = Arc::new(AtomicUsize::new(0));

		let fetcher = TestExecutor {
			values: HashMap::from_iter(vec![("a", 1), ("b", 2), ("c", 3)]),
			delay: std::time::Duration::from_millis(5),
			requests: requests.clone(),
			capacity: 2,
		};

		let loader = BatcherBuilder::default()
			.batch_size(2)
			.concurrency(100)
			.delay(std::time::Duration::from_millis(10))
			.build(fetcher);

		tokio::time::sleep(std::time::Duration::from_millis(5)).await;

		let start = std::time::Instant::now();
		let ab = loader.execute_many(vec!["a"]).await;
		assert_eq!(ab, vec![Some(1)]);
		assert!(start.elapsed() >= std::time::Duration::from_millis(15));
		assert!(start.elapsed() < std::time::Duration::from_millis(20));
	}

	#[tokio::test]
	async fn no_deduplication() {
		let requests = Arc::new(AtomicUsize::new(0));

		let fetcher = TestExecutor {
			values: HashMap::from_iter(vec![("a", 1), ("b", 2), ("c", 3)]),
			delay: std::time::Duration::from_millis(5),
			requests: requests.clone(),
			capacity: 4,
		};

		let loader = BatcherBuilder::default()
			.batch_size(4)
			.concurrency(1)
			.delay(std::time::Duration::from_millis(10))
			.build(fetcher);

		let start = std::time::Instant::now();
		let ab = loader.execute_many(vec!["a", "a", "b", "b", "c", "c"]).await;
		assert_eq!(ab, vec![Some(1), Some(1), Some(2), Some(2), Some(3), Some(3)]);
		assert_eq!(requests.load(std::sync::atomic::Ordering::Relaxed), 2);
		assert!(start.elapsed() >= std::time::Duration::from_millis(5));
		assert!(start.elapsed() < std::time::Duration::from_millis(20));
	}

	#[tokio::test]
	async fn result() {
		let requests = Arc::new(AtomicUsize::new(0));

		struct TestExecutor(Arc<AtomicUsize>);

		impl BatchExecutor for TestExecutor {
			type Request = &'static str;
			type Response = Result<usize, ()>;

			async fn execute(&self, requests: Vec<(Self::Request, BatchResponse<Self::Response>)>) {
				self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
				for (request, response) in requests {
					match request.parse() {
						Ok(value) => response.send_ok(value),
						Err(_) => response.send_err(()),
					}
				}
			}
		}

		let loader = BatcherBuilder::default()
			.batch_size(4)
			.concurrency(1)
			.delay(std::time::Duration::from_millis(10))
			.build(TestExecutor(requests.clone()));

		let start = std::time::Instant::now();
		let ab = loader.execute_many(vec!["1", "1", "2", "2", "3", "3", "hello"]).await;
		assert_eq!(
			ab,
			vec![
				Some(Ok(1)),
				Some(Ok(1)),
				Some(Ok(2)),
				Some(Ok(2)),
				Some(Ok(3)),
				Some(Ok(3)),
				Some(Err(()))
			]
		);
		assert_eq!(requests.load(std::sync::atomic::Ordering::Relaxed), 2);
		assert!(start.elapsed() >= std::time::Duration::from_millis(5));
		assert!(start.elapsed() < std::time::Duration::from_millis(20));
	}

	#[tokio::test]
	async fn option() {
		let requests = Arc::new(AtomicUsize::new(0));

		struct TestExecutor(Arc<AtomicUsize>);

		impl BatchExecutor for TestExecutor {
			type Request = &'static str;
			type Response = Option<usize>;

			async fn execute(&self, requests: Vec<(Self::Request, BatchResponse<Self::Response>)>) {
				self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
				for (request, response) in requests {
					match request.parse() {
						Ok(value) => response.send_some(value),
						Err(_) => response.send_none(),
					}
				}
			}
		}

		let loader = BatcherBuilder::default()
			.batch_size(4)
			.concurrency(1)
			.delay(std::time::Duration::from_millis(10))
			.build(TestExecutor(requests.clone()));

		let start = std::time::Instant::now();
		let ab = loader.execute_many(vec!["1", "1", "2", "2", "3", "3", "hello"]).await;
		assert_eq!(
			ab,
			vec![
				Some(Some(1)),
				Some(Some(1)),
				Some(Some(2)),
				Some(Some(2)),
				Some(Some(3)),
				Some(Some(3)),
				Some(None)
			]
		);
		assert_eq!(requests.load(std::sync::atomic::Ordering::Relaxed), 2);
		assert!(start.elapsed() >= std::time::Duration::from_millis(5));
		assert!(start.elapsed() < std::time::Duration::from_millis(20));
	}
}
