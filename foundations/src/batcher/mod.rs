use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasher, RandomState};
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, AtomicUsize};
use std::sync::Arc;

use tokio::sync::OnceCell;
use tracing::Instrument;

pub mod dataloader;

pub trait BatchMode<T: BatchOperation + ?Sized>: Sized {
	type Input: Send + Sync;
	type Output: Send + Sync;
	type OutputItem: Send + Sync;
	type OperationOutput: Send + Sync;
	type FinalOutput: Send + Sync;
	type Tracker: Send + Sync;

	fn new_input() -> Self::Input;
	fn new_tracker() -> Self::Tracker;
	fn new_output() -> Self::Output;

	fn input_add(input: &mut Self::Input, tracker: &mut Self::Tracker, item: T::Item);
	fn input_len(input: &Self::Input) -> usize;

	fn tracked_output(
		result: Option<&Result<Self::OperationOutput, BatcherError<T::Error>>>,
		tracker: Self::Tracker,
		output: &mut Self::Output,
	) -> Result<(), BatcherError<T::Error>>;

	fn final_output_into_iter(
		output: Self::FinalOutput,
	) -> Result<impl IntoIterator<Item = Self::OutputItem>, BatcherError<T::Error>>;

	fn filter_item_iter(item: impl IntoIterator<Item = T::Item>) -> impl IntoIterator<Item = T::Item>;

	fn output_item_to_result(item: Self::OutputItem) -> Result<T::Response, BatcherError<T::Error>>;

	fn output_into_final_output(output: Result<Self::Output, BatcherError<T::Error>>) -> Self::FinalOutput;
}

pub struct BatcherNormalMode;

impl<T: BatchOperation> BatchMode<T> for BatcherNormalMode {
	type FinalOutput = Self::Output;
	type Input = Vec<T::Item>;
	type OperationOutput = Vec<Result<T::Response, T::Error>>;
	type Output = Vec<Self::OutputItem>;
	type OutputItem = Result<T::Response, BatcherError<T::Error>>;
	type Tracker = std::ops::Range<usize>;

	fn new_input() -> Self::Input {
		Vec::new()
	}

	fn new_tracker() -> Self::Tracker {
		0..0
	}

	fn new_output() -> Self::Output {
		Vec::new()
	}

	fn input_add(input: &mut Self::Input, tracker: &mut Self::Tracker, item: T::Item) {
		input.push(item);
		tracker.end = input.len();
	}

	fn input_len(input: &Self::Input) -> usize {
		input.len()
	}

	fn tracked_output(
		result: Option<&Result<Self::OperationOutput, BatcherError<T::Error>>>,
		tracker: Self::Tracker,
		output: &mut Self::Output,
	) -> Result<(), BatcherError<T::Error>> {
		for i in tracker.into_iter() {
			match result {
				Some(Ok(r)) => output.push(
					r.get(i)
						.cloned()
						.transpose()
						.map_err(BatcherError::Batch)
						.transpose()
						.unwrap_or(Err(BatcherError::MissingResult)),
				),
				Some(Err(e)) => output.push(Err(e.clone())),
				None => output.push(Err(BatcherError::Panic)),
			}
		}

		Ok(())
	}

	fn final_output_into_iter(
		output: Self::FinalOutput,
	) -> Result<impl IntoIterator<Item = Self::OutputItem>, BatcherError<T::Error>> {
		Ok(output)
	}

	fn filter_item_iter(item: impl IntoIterator<Item = T::Item>) -> impl IntoIterator<Item = T::Item> {
		item
	}

	fn output_item_to_result(item: Self::OutputItem) -> Result<T::Response, BatcherError<T::Error>> {
		item
	}

	fn output_into_final_output(
		output: Result<Self::Output, BatcherError<<T as BatchOperation>::Error>>,
	) -> Self::FinalOutput {
		output.expect("erro shouldnt be possible here")
	}
}

pub struct BatcherDataloader<S: BuildHasher + Default + Send + Sync = RandomState>(PhantomData<S>);

impl<T: BatchOperation, S: BuildHasher + Default + Send + Sync> BatchMode<T> for BatcherDataloader<S>
where
	T::Item: Clone + std::hash::Hash + Eq,
{
	type FinalOutput = Result<HashMap<T::Item, Self::OutputItem, S>, BatcherError<T::Error>>;
	type Input = HashSet<T::Item, S>;
	type OperationOutput = HashMap<T::Item, Self::OutputItem, S>;
	type Output = Self::OperationOutput;
	type OutputItem = T::Response;
	type Tracker = Vec<T::Item>;

	fn new_input() -> Self::Input {
		HashSet::default()
	}

	fn new_tracker() -> Self::Tracker {
		Vec::new()
	}

	fn new_output() -> Self::Output {
		HashMap::default()
	}

	fn input_add(input: &mut Self::Input, tracker: &mut Self::Tracker, item: T::Item) {
		input.insert(item.clone());
		tracker.push(item);
	}

	fn input_len(input: &Self::Input) -> usize {
		input.len()
	}

	fn tracked_output(
		result: Option<&Result<Self::OperationOutput, BatcherError<T::Error>>>,
		tracker: Self::Tracker,
		output: &mut Self::Output,
	) -> Result<(), BatcherError<T::Error>> {
		for key in tracker.clone().into_iter() {
			match result {
				Some(Ok(res)) => {
					if let Some(value) = res.get(&key).cloned() {
						output.insert(key, value);
					}
				}
				Some(Err(e)) => {
					return Err(e.clone());
				}
				None => {
					return Err(BatcherError::Panic);
				}
			}
		}

		Ok(())
	}

	fn final_output_into_iter(
		output: Self::FinalOutput,
	) -> Result<impl IntoIterator<Item = Self::OutputItem>, BatcherError<T::Error>> {
		output.map(|output| output.into_values())
	}

	fn filter_item_iter(item: impl IntoIterator<Item = T::Item>) -> impl IntoIterator<Item = T::Item> {
		item
	}

	fn output_item_to_result(item: Self::OutputItem) -> Result<T::Response, BatcherError<T::Error>> {
		Ok(item)
	}

	fn output_into_final_output(
		output: Result<Self::Output, BatcherError<<T as BatchOperation>::Error>>,
	) -> Self::FinalOutput {
		output
	}
}

pub trait BatchOperation {
	type Item: Send + Sync;
	type Response: Clone + Send + Sync;
	type Error: Clone + std::fmt::Debug + Send + Sync;
	type Mode: BatchMode<Self>;

	fn config(&self) -> BatcherConfig;

	fn process(
		&self,
		documents: <Self::Mode as BatchMode<Self>>::Input,
	) -> impl std::future::Future<Output = Result<<Self::Mode as BatchMode<Self>>::OperationOutput, Self::Error>> + Send + '_
	where
		Self: Send + Sync;
}

pub struct Batcher<T: BatchOperation> {
	inner: Arc<BatcherInner<T>>,
	_auto_loader_abort: CancelOnDrop,
}

struct CancelOnDrop(tokio::task::AbortHandle);

impl Drop for CancelOnDrop {
	fn drop(&mut self) {
		self.0.abort();
	}
}

struct BatcherInner<T: BatchOperation> {
	semaphore: tokio::sync::Semaphore,
	notify: tokio::sync::Notify,
	sleep_duration: AtomicU64,
	batch_id: AtomicU64,
	max_batch_size: AtomicUsize,
	operation: T,
	name: String,
	active_batch: tokio::sync::RwLock<Option<Batch<T>>>,
}

struct Batch<T: BatchOperation> {
	id: u64,
	expires_at: tokio::time::Instant,
	done: DropGuardCancellationToken,
	ops: <T::Mode as BatchMode<T>>::Input,
	#[allow(clippy::type_complexity)]
	results: Arc<OnceCell<Result<<T::Mode as BatchMode<T>>::OperationOutput, BatcherError<T::Error>>>>,
}

struct DropGuardCancellationToken(tokio_util::sync::CancellationToken);

impl Drop for DropGuardCancellationToken {
	fn drop(&mut self) {
		self.0.cancel();
	}
}

impl DropGuardCancellationToken {
	fn new() -> Self {
		Self(tokio_util::sync::CancellationToken::new())
	}

	fn child_token(&self) -> tokio_util::sync::CancellationToken {
		self.0.child_token()
	}
}

struct BatchInsertWaiter<T: BatchOperation> {
	id: u64,
	done: tokio_util::sync::CancellationToken,
	tracker: <T::Mode as BatchMode<T>>::Tracker,
	#[allow(clippy::type_complexity)]
	results: Arc<OnceCell<Result<<T::Mode as BatchMode<T>>::OperationOutput, BatcherError<T::Error>>>>,
}

#[derive(thiserror::Error, Debug, Clone, PartialEq, Copy, Eq, Hash, Ord, PartialOrd)]
pub enum BatcherError<E> {
	#[error("failed to acquire semaphore")]
	AcquireSemaphore,
	#[error("panic in batch inserter")]
	Panic,
	#[error("missing result")]
	MissingResult,
	#[error("batch failed with: {0}")]
	Batch(E),
}

impl<E: std::error::Error> From<E> for BatcherError<E> {
	fn from(value: E) -> Self {
		Self::Batch(value)
	}
}

impl<T: BatchOperation + 'static + Send + Sync> Batch<T> {
	#[tracing::instrument(skip_all, fields(name = %inner.name), level = "debug")]
	async fn run(self, inner: Arc<BatcherInner<T>>) {
		self.results
			.get_or_init(|| async move {
				let _ticket = inner
					.semaphore
					.acquire()
					.instrument(tracing::debug_span!("Semaphore"))
					.await
					.map_err(|_| BatcherError::AcquireSemaphore)?;
				inner.operation.process(self.ops).await.map_err(BatcherError::Batch)
			})
			.await;
	}
}

#[derive(Clone)]
pub struct BatcherConfig {
	pub name: String,
	pub concurrency: usize,
	pub max_batch_size: usize,
	pub sleep_duration: std::time::Duration,
}

impl<T: BatchOperation + 'static + Send + Sync> BatcherInner<T> {
	fn spawn_batch(self: &Arc<Self>, batch: Batch<T>) {
		tokio::spawn(batch.run(self.clone()));
	}

	fn new_batch(&self) -> Batch<T> {
		let id = self.batch_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
		let expires_at = tokio::time::Instant::now()
			+ tokio::time::Duration::from_nanos(self.sleep_duration.load(std::sync::atomic::Ordering::Relaxed));

		Batch {
			id,
			expires_at,
			ops: T::Mode::new_input(),
			done: DropGuardCancellationToken::new(),
			results: Arc::new(OnceCell::new()),
		}
	}

	async fn batch_inserts(self: &Arc<Self>, documents: impl IntoIterator<Item = T::Item>) -> Vec<BatchInsertWaiter<T>> {
		let mut waiters = vec![];
		let mut batch = self.active_batch.write().await;
		let max_documents = self.max_batch_size.load(std::sync::atomic::Ordering::Relaxed);

		for document in T::Mode::filter_item_iter(documents) {
			if batch
				.as_ref()
				.map(|b| T::Mode::input_len(&b.ops) >= max_documents)
				.unwrap_or(true)
			{
				if let Some(b) = batch.take() {
					self.spawn_batch(b);
				}

				*batch = Some(self.new_batch());
				self.notify.notify_one();
			}

			let Some(b) = batch.as_mut() else {
				unreachable!("batch should be Some");
			};

			if waiters.last().map(|w: &BatchInsertWaiter<T>| w.id != b.id).unwrap_or(true) {
				waiters.push(BatchInsertWaiter {
					id: b.id,
					done: b.done.child_token(),
					results: b.results.clone(),
					tracker: T::Mode::new_tracker(),
				});
			}

			let tracker = &mut waiters.last_mut().unwrap().tracker;
			T::Mode::input_add(&mut b.ops, tracker, document);
		}

		waiters
	}
}

impl<T: BatchOperation + 'static + Send + Sync> Batcher<T> {
	pub fn new(operation: T) -> Self {
		let config = operation.config();

		let inner = Arc::new(BatcherInner {
			semaphore: tokio::sync::Semaphore::new(config.concurrency),
			notify: tokio::sync::Notify::new(),
			batch_id: AtomicU64::new(0),
			active_batch: tokio::sync::RwLock::new(None),
			sleep_duration: AtomicU64::new(config.sleep_duration.as_nanos() as u64),
			max_batch_size: AtomicUsize::new(config.max_batch_size),
			operation,
			name: config.name,
		});

		Self {
			inner: inner.clone(),
			_auto_loader_abort: CancelOnDrop(
				tokio::task::spawn(async move {
					loop {
						inner.notify.notified().await;
						let Some((id, expires_at)) = inner.active_batch.read().await.as_ref().map(|b| (b.id, b.expires_at))
						else {
							continue;
						};

						if expires_at > tokio::time::Instant::now() {
							tokio::time::sleep_until(expires_at).await;
						}

						let mut batch = inner.active_batch.write().await;
						if batch.as_ref().is_some_and(|b| b.id == id) {
							inner.spawn_batch(batch.take().unwrap());
						}
					}
				})
				.abort_handle(),
			),
		}
	}

	#[tracing::instrument(skip_all, fields(name = %self.inner.name))]
	pub async fn execute(&self, document: T::Item) -> Result<T::Response, BatcherError<T::Error>> {
		let output = self.internal_execute_many(std::iter::once(document)).await;
		let iter = T::Mode::final_output_into_iter(output)?;
		T::Mode::output_item_to_result(iter.into_iter().next().ok_or(BatcherError::MissingResult)?)
	}

	#[tracing::instrument(skip_all, fields(name = %self.inner.name))]
	pub async fn execute_many(
		&self,
		documents: impl IntoIterator<Item = T::Item>,
	) -> <T::Mode as BatchMode<T>>::FinalOutput {
		self.internal_execute_many(documents).await
	}

	pub async fn internal_execute_many(
		&self,
		documents: impl IntoIterator<Item = T::Item>,
	) -> <T::Mode as BatchMode<T>>::FinalOutput {
		let waiters = self.inner.batch_inserts(documents).await;

		let mut results = <T::Mode as BatchMode<T>>::new_output();

		for waiter in waiters {
			waiter.done.cancelled().await;
			if let Err(e) = <T::Mode as BatchMode<T>>::tracked_output(waiter.results.get(), waiter.tracker, &mut results) {
				return <T::Mode as BatchMode<T>>::output_into_final_output(Err(e));
			}
		}

		<T::Mode as BatchMode<T>>::output_into_final_output(Ok(results))
	}
}
