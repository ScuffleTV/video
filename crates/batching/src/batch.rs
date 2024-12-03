use std::future::Future;
use std::sync::atomic::AtomicU64;
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
pub struct BatcherBuilder {
	batch_size: usize,
	concurrency: usize,
	delay: std::time::Duration,
}

impl Default for BatcherBuilder {
	fn default() -> Self {
		Self::new()
	}
}

impl BatcherBuilder {
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

	/// Set the concurrency to 1
	pub fn concurrency(mut self, concurrency: usize) -> Self {
		self.concurrency = concurrency;
		self
	}

	/// Set the concurrency
	pub fn with_concurrency(&mut self, concurrency: usize) -> &mut Self {
		self.concurrency = concurrency;
		self
	}

	/// Set the batch size
	pub fn with_batch_size(&mut self, batch_size: usize) -> &mut Self {
		self.batch_size = batch_size;
		self
	}

	/// Set the delay
	pub fn with_delay(&mut self, delay: std::time::Duration) -> &mut Self {
		self.delay = delay;
		self
	}

	/// Build the batcher
	pub fn build<E>(self, executor: E) -> Batcher<E>
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
	batch_id: AtomicU64,
}

struct Batch<E>
where
	E: BatchExecutor + Send + Sync + 'static,
{
	id: u64,
	items: Vec<(E::Request, BatchResponse<E::Response>)>,
	semaphore: Arc<tokio::sync::Semaphore>,
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
			batch_id: AtomicU64::new(0),
		}
	}

	/// Create a builder for a [`Batcher`]
	pub fn builder() -> BatcherBuilder {
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
					batch.replace(
						Batch::new(
							self.batch_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
							self.semaphore.clone(),
						),
					);
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

impl<E> Batch<E>
where
	E: BatchExecutor + Send + Sync + 'static,
{
	fn new(id: u64, semaphore: Arc<tokio::sync::Semaphore>) -> Self {
		Self {
			id,
			items: Vec::new(),
			semaphore,
		}
	}

	async fn spawn(self, executor: Arc<E>) {
		let _ticket = self.semaphore.acquire_owned().await;
		executor.execute(self.items).await;
	}
}
