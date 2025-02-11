use std::collections::{HashMap, HashSet};
use std::future::Future;
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
pub struct DataLoaderBuilder<E> {
    batch_size: usize,
    concurrency: usize,
    delay: std::time::Duration,
    _phantom: std::marker::PhantomData<E>,
}

impl<E> Default for DataLoaderBuilder<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E> DataLoaderBuilder<E> {
    /// Create a new builder
    pub const fn new() -> Self {
        Self {
            batch_size: 1000,
            concurrency: 50,
            delay: std::time::Duration::from_millis(5),
            _phantom: std::marker::PhantomData,
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

    /// Set the concurrency
    #[inline]
    pub const fn concurrency(mut self, concurrency: usize) -> Self {
        self.with_concurrency(concurrency);
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

    /// Set the concurrency
    #[inline]
    pub const fn with_concurrency(&mut self, concurrency: usize) -> &mut Self {
        self.concurrency = concurrency;
        self
    }

    /// Build the dataloader
    #[inline]
    pub fn build(self, executor: E) -> DataLoader<E>
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
        }
    }

    /// Create a builder for a [`DataLoader`]
    #[inline]
    pub const fn builder() -> DataLoaderBuilder<E> {
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
            keys: HashSet<K>,
            result: Arc<BatchResult<K, V>>,
        }

        let mut waiters = Vec::<BatchWaiting<E::Key, E::Value>>::new();

        let mut count = 0;

        {
            let mut new_batch = true;
            let mut batch = self.current_batch.lock().await;

            for item in items {
                if batch.is_none() {
                    batch.replace(Batch::new(self.semaphore.clone()));
                    new_batch = true;
                }

                let batch_mut = batch.as_mut().unwrap();
                batch_mut.items.insert(item.clone());

                if new_batch {
                    new_batch = false;
                    waiters.push(BatchWaiting {
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
    items: HashSet<E::Key>,
    result: Arc<BatchResult<E::Key, E::Value>>,
    semaphore: Arc<tokio::sync::Semaphore>,
    created_at: std::time::Instant,
}

impl<E> Batch<E>
where
    E: DataLoaderFetcher + Send + Sync + 'static,
{
    fn new(semaphore: Arc<tokio::sync::Semaphore>) -> Self {
        Self {
            items: HashSet::new(),
            result: Arc::new(BatchResult::new()),
            semaphore,
            created_at: std::time::Instant::now(),
        }
    }

    async fn spawn(self, executor: Arc<E>) {
        let _drop_guard = self.result.token.clone().drop_guard();
        let _ticket = self.semaphore.acquire_owned().await.unwrap();
        let result = executor.load(self.items).await;

        #[cfg_attr(all(coverage_nightly, test), coverage(off))]
        fn unknwown_error<E>(_: E) -> ! {
            unreachable!(
                "batch result already set, this is a bug please report it https://github.com/scufflecloud/scuffle/issues"
            )
        }

        self.result.values.set(result).map_err(unknwown_error).unwrap();
    }
}

#[cfg_attr(all(coverage_nightly, test), coverage(off))]
#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;

    use super::*;

    struct TestFetcher<K, V> {
        values: HashMap<K, V>,
        delay: std::time::Duration,
        requests: Arc<AtomicUsize>,
        capacity: usize,
    }

    impl<K, V> DataLoaderFetcher for TestFetcher<K, V>
    where
        K: Clone + Eq + std::hash::Hash + Send + Sync,
        V: Clone + Send + Sync,
    {
        type Key = K;
        type Value = V;

        async fn load(&self, keys: HashSet<Self::Key>) -> Option<HashMap<Self::Key, Self::Value>> {
            assert!(keys.len() <= self.capacity);
            tokio::time::sleep(self.delay).await;
            self.requests.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Some(
                keys.into_iter()
                    .filter_map(|k| {
                        let value = self.values.get(&k)?.clone();
                        Some((k, value))
                    })
                    .collect(),
            )
        }
    }

    #[cfg(not(valgrind))] // test is time-sensitive
    #[tokio::test]
    async fn basic() {
        let requests = Arc::new(AtomicUsize::new(0));

        let fetcher = TestFetcher {
            values: HashMap::from_iter(vec![("a", 1), ("b", 2), ("c", 3)]),
            delay: std::time::Duration::from_millis(5),
            requests: requests.clone(),
            capacity: 2,
        };

        let loader = DataLoader::builder().batch_size(2).concurrency(1).build(fetcher);

        let start = std::time::Instant::now();
        let a = loader.load("a").await.unwrap();
        assert_eq!(a, Some(1));
        assert!(start.elapsed() < std::time::Duration::from_millis(15));
        assert_eq!(requests.load(std::sync::atomic::Ordering::Relaxed), 1);

        let start = std::time::Instant::now();
        let b = loader.load("b").await.unwrap();
        assert_eq!(b, Some(2));
        assert!(start.elapsed() < std::time::Duration::from_millis(15));
        assert_eq!(requests.load(std::sync::atomic::Ordering::Relaxed), 2);
        let start = std::time::Instant::now();
        let c = loader.load("c").await.unwrap();
        assert_eq!(c, Some(3));
        assert!(start.elapsed() < std::time::Duration::from_millis(15));
        assert_eq!(requests.load(std::sync::atomic::Ordering::Relaxed), 3);

        let start = std::time::Instant::now();
        let ab = loader.load_many(vec!["a", "b"]).await.unwrap();
        assert_eq!(ab, HashMap::from_iter(vec![("a", 1), ("b", 2)]));
        assert!(start.elapsed() < std::time::Duration::from_millis(15));
        assert_eq!(requests.load(std::sync::atomic::Ordering::Relaxed), 4);

        let start = std::time::Instant::now();
        let unknown = loader.load("unknown").await.unwrap();
        assert_eq!(unknown, None);
        assert!(start.elapsed() < std::time::Duration::from_millis(15));
        assert_eq!(requests.load(std::sync::atomic::Ordering::Relaxed), 5);
    }

    #[cfg(not(valgrind))] // test is time-sensitive
    #[tokio::test]
    async fn concurrency_high() {
        let requests = Arc::new(AtomicUsize::new(0));

        let fetcher = TestFetcher {
            values: HashMap::from_iter(vec![("a", 1), ("b", 2), ("c", 3)]),
            delay: std::time::Duration::from_millis(5),
            requests: requests.clone(),
            capacity: 2,
        };

        let loader = DataLoader::builder().batch_size(2).concurrency(10).build(fetcher);

        let start = std::time::Instant::now();
        let ab = loader
            .load_many(vec!["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"])
            .await
            .unwrap();
        assert_eq!(ab, HashMap::from_iter(vec![("a", 1), ("b", 2), ("c", 3)]));
        assert!(start.elapsed() < std::time::Duration::from_millis(15));
        assert_eq!(requests.load(std::sync::atomic::Ordering::Relaxed), 5);
    }

    #[cfg(not(valgrind))] // test is time-sensitive
    #[tokio::test]
    async fn delay_low() {
        let requests = Arc::new(AtomicUsize::new(0));

        let fetcher = TestFetcher {
            values: HashMap::from_iter(vec![("a", 1), ("b", 2), ("c", 3)]),
            delay: std::time::Duration::from_millis(5),
            requests: requests.clone(),
            capacity: 2,
        };

        let loader = DataLoader::builder()
            .batch_size(2)
            .concurrency(1)
            .delay(std::time::Duration::from_millis(10))
            .build(fetcher);

        let start = std::time::Instant::now();
        let ab = loader
            .load_many(vec!["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"])
            .await
            .unwrap();
        assert_eq!(ab, HashMap::from_iter(vec![("a", 1), ("b", 2), ("c", 3)]));
        assert!(start.elapsed() < std::time::Duration::from_millis(35));
        assert!(start.elapsed() >= std::time::Duration::from_millis(25));
        assert_eq!(requests.load(std::sync::atomic::Ordering::Relaxed), 5);
    }

    #[cfg(not(valgrind))] // test is time-sensitive
    #[tokio::test]
    async fn batch_size() {
        let requests = Arc::new(AtomicUsize::new(0));

        let fetcher = TestFetcher {
            values: HashMap::from_iter(vec![("a", 1), ("b", 2), ("c", 3)]),
            delay: std::time::Duration::from_millis(5),
            requests: requests.clone(),
            capacity: 100,
        };

        let loader = DataLoaderBuilder::default()
            .batch_size(100)
            .concurrency(1)
            .delay(std::time::Duration::from_millis(10))
            .build(fetcher);

        let start = std::time::Instant::now();
        let ab = loader
            .load_many(vec!["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"])
            .await
            .unwrap();
        assert_eq!(ab, HashMap::from_iter(vec![("a", 1), ("b", 2), ("c", 3)]));
        assert!(start.elapsed() >= std::time::Duration::from_millis(10));
        assert_eq!(requests.load(std::sync::atomic::Ordering::Relaxed), 1);
    }

    #[cfg(not(valgrind))] // test is time-sensitive
    #[tokio::test]
    async fn high_concurrency() {
        let requests = Arc::new(AtomicUsize::new(0));

        let fetcher = TestFetcher {
            values: HashMap::from_iter((0..1134).map(|i| (i, i * 2 + 5))),
            delay: std::time::Duration::from_millis(5),
            requests: requests.clone(),
            capacity: 100,
        };

        let loader = DataLoaderBuilder::default()
            .batch_size(100)
            .concurrency(10)
            .delay(std::time::Duration::from_millis(10))
            .build(fetcher);

        let start = std::time::Instant::now();
        let ab = loader.load_many(0..1134).await.unwrap();
        assert_eq!(ab, HashMap::from_iter((0..1134).map(|i| (i, i * 2 + 5))));
        assert!(start.elapsed() >= std::time::Duration::from_millis(15));
        assert!(start.elapsed() < std::time::Duration::from_millis(25));
        assert_eq!(requests.load(std::sync::atomic::Ordering::Relaxed), 1134 / 100 + 1);
    }

    #[cfg(not(valgrind))] // test is time-sensitive
    #[tokio::test]
    async fn delayed_start() {
        let requests = Arc::new(AtomicUsize::new(0));

        let fetcher = TestFetcher {
            values: HashMap::from_iter(vec![("a", 1), ("b", 2), ("c", 3)]),
            delay: std::time::Duration::from_millis(5),
            requests: requests.clone(),
            capacity: 2,
        };

        let loader = DataLoader::builder()
            .batch_size(2)
            .concurrency(100)
            .delay(std::time::Duration::from_millis(10))
            .build(fetcher);

        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        let start = std::time::Instant::now();
        let ab = loader
            .load_many(vec!["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"])
            .await
            .unwrap();
        assert_eq!(ab, HashMap::from_iter(vec![("a", 1), ("b", 2), ("c", 3)]));
        assert!(start.elapsed() >= std::time::Duration::from_millis(5));
        assert!(start.elapsed() < std::time::Duration::from_millis(25));
    }

    #[cfg(not(valgrind))] // test is time-sensitive
    #[tokio::test]
    async fn delayed_start_single() {
        let requests = Arc::new(AtomicUsize::new(0));

        let fetcher = TestFetcher {
            values: HashMap::from_iter(vec![("a", 1), ("b", 2), ("c", 3)]),
            delay: std::time::Duration::from_millis(5),
            requests: requests.clone(),
            capacity: 2,
        };

        let loader = DataLoader::builder()
            .batch_size(2)
            .concurrency(100)
            .delay(std::time::Duration::from_millis(10))
            .build(fetcher);

        tokio::time::sleep(std::time::Duration::from_millis(5)).await;

        let start = std::time::Instant::now();
        let ab = loader.load_many(vec!["a"]).await.unwrap();
        assert_eq!(ab, HashMap::from_iter(vec![("a", 1)]));
        assert!(start.elapsed() >= std::time::Duration::from_millis(15));
        assert!(start.elapsed() < std::time::Duration::from_millis(20));
    }

    #[cfg(not(valgrind))] // test is time-sensitive
    #[tokio::test]
    async fn deduplication() {
        let requests = Arc::new(AtomicUsize::new(0));

        let fetcher = TestFetcher {
            values: HashMap::from_iter(vec![("a", 1), ("b", 2), ("c", 3)]),
            delay: std::time::Duration::from_millis(5),
            requests: requests.clone(),
            capacity: 4,
        };

        let loader = DataLoader::builder()
            .batch_size(4)
            .concurrency(1)
            .delay(std::time::Duration::from_millis(10))
            .build(fetcher);

        let start = std::time::Instant::now();
        let ab = loader.load_many(vec!["a", "a", "b", "b", "c", "c"]).await.unwrap();
        assert_eq!(ab, HashMap::from_iter(vec![("a", 1), ("b", 2), ("c", 3)]));
        assert_eq!(requests.load(std::sync::atomic::Ordering::Relaxed), 1);
        assert!(start.elapsed() >= std::time::Duration::from_millis(5));
        assert!(start.elapsed() < std::time::Duration::from_millis(20));
    }

    #[cfg(not(valgrind))] // test is time-sensitive
    #[tokio::test]
    async fn already_batch() {
        let requests = Arc::new(AtomicUsize::new(0));

        let fetcher = TestFetcher {
            values: HashMap::from_iter(vec![("a", 1), ("b", 2), ("c", 3)]),
            delay: std::time::Duration::from_millis(5),
            requests: requests.clone(),
            capacity: 2,
        };

        let loader = DataLoader::builder().batch_size(10).concurrency(1).build(fetcher);

        let start = std::time::Instant::now();
        let (a, b) = tokio::join!(loader.load("a"), loader.load("b"));
        assert_eq!(a, Ok(Some(1)));
        assert_eq!(b, Ok(Some(2)));
        assert!(start.elapsed() < std::time::Duration::from_millis(15));
        assert_eq!(requests.load(std::sync::atomic::Ordering::Relaxed), 1);
    }
}
