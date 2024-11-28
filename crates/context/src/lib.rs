#![doc = include_str!("../README.md")]

use std::future::{Future, IntoFuture};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::Arc;
use std::task::Poll;

use futures_lite::Stream;
use tokio_util::sync::{CancellationToken, WaitForCancellationFuture, WaitForCancellationFutureOwned};

#[derive(Debug)]
struct ContextTracker(Arc<ContextTrackerInner>);

impl Drop for ContextTracker {
	fn drop(&mut self) {
		let remaining = self.0.active_count.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
		if remaining == 1 && self.0.stopped.load(std::sync::atomic::Ordering::Relaxed) {
			self.0.notify.notify_waiters();
		}
	}
}

#[derive(Debug)]
struct ContextTrackerInner {
	stopped: AtomicBool,
	active_count: AtomicUsize,
	notify: tokio::sync::Notify,
}

impl ContextTrackerInner {
	fn new() -> Arc<Self> {
		Arc::new(Self {
			stopped: AtomicBool::new(false),
			active_count: AtomicUsize::new(0),
			notify: tokio::sync::Notify::new(),
		})
	}

	fn child(self: &Arc<Self>) -> ContextTracker {
		self.active_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
		ContextTracker(self.clone())
	}

	fn stop(&self) {
		self.stopped.store(true, std::sync::atomic::Ordering::Relaxed);
	}

	async fn wait(&self) {
		let notify = self.notify.notified();

		// If there are no active children, then the notify will never be called
		if self.active_count.load(std::sync::atomic::Ordering::Relaxed) == 0 {
			return;
		}

		notify.await;
	}
}

/// A context for cancelling futures and waiting for shutdown
///
/// A context can be created from a handler or another context so to have a
/// hierarchy of contexts
///
/// Contexts can then be attached to futures or streams in order to
/// automatically cancel them when the context is done, when invoking
/// `Handler::cancel`. The `Handler::shutdown` method will block until all
/// contexts have been dropped allowing for a graceful shutdown.
#[derive(Debug)]
pub struct Context {
	token: CancellationToken,
	tracker: ContextTracker,
}

impl Clone for Context {
	fn clone(&self) -> Self {
		Self {
			token: self.token.clone(),
			tracker: self.tracker.0.child(),
		}
	}
}

impl Context {
	#[must_use]
	/// Create a new context using the global handler
	/// Returns a tuple and a child handler
	pub fn new() -> (Self, Handler) {
		Handler::global().new_child()
	}

	#[must_use]
	/// Create a new child context from this context
	/// Returns a tuple and a child handler
	pub fn new_child(&self) -> (Self, Handler) {
		let token = self.token.child_token();
		let tracker = ContextTrackerInner::new();

		(
			Self {
				tracker: tracker.child(),
				token: token.clone(),
			},
			Handler {
				token: Arc::new(TokenDropGuard(token)),
				tracker,
			},
		)
	}

	#[must_use]
	/// Returns the global context
	pub fn global() -> Self {
		Handler::global().context()
	}

	/// Waits for the context to be done (the handler to be shutdown)
	pub async fn done(&self) {
		self.token.cancelled().await;
	}

	/// The same as done but takes ownership of the context
	pub async fn into_done(self) {
		self.done().await;
	}

	/// Returns true if the context is done
	#[must_use]
	pub fn is_done(&self) -> bool {
		self.token.is_cancelled()
	}
}

#[derive(Debug)]
struct TokenDropGuard(CancellationToken);

impl TokenDropGuard {
	#[must_use]
	fn child(&self) -> CancellationToken {
		self.0.child_token()
	}

	fn cancel(&self) {
		self.0.cancel();
	}
}

impl Drop for TokenDropGuard {
	fn drop(&mut self) {
		self.cancel();
	}
}

#[derive(Debug, Clone)]
pub struct Handler {
	token: Arc<TokenDropGuard>,
	tracker: Arc<ContextTrackerInner>,
}

impl Default for Handler {
	fn default() -> Self {
		Self::new()
	}
}

impl Handler {
	#[must_use]
	/// Create a new handler
	pub fn new() -> Handler {
		let token = CancellationToken::new();
		let tracker = ContextTrackerInner::new();

		Handler {
			token: Arc::new(TokenDropGuard(token)),
			tracker,
		}
	}

	#[must_use]
	/// Returns the global handler
	pub fn global() -> &'static Self {
		static GLOBAL: std::sync::OnceLock<Handler> = std::sync::OnceLock::new();

		GLOBAL.get_or_init(Handler::new)
	}

	/// Shutdown the handler and wait for all contexts to be done
	pub async fn shutdown(&self) {
		self.cancel();
		self.done().await;
	}

	/// Waits for the handler to be done (waiting for all contexts to be done)
	pub async fn done(&self) {
		self.token.0.cancelled().await;
		self.tracker.wait().await;
	}

	/// Waits for the handler to be done (waiting for all contexts to be done)
	/// Returns once all contexts are done, even if the handler is not done and
	/// contexts can be created after this call.
	pub async fn wait(&self) {
		self.tracker.wait().await;
	}

	#[must_use]
	/// Create a new context from this handler
	pub fn context(&self) -> Context {
		Context {
			token: self.token.child(),
			tracker: self.tracker.child(),
		}
	}

	#[must_use]
	/// Create a new child context from this handler
	pub fn new_child(&self) -> (Context, Handler) {
		self.context().new_child()
	}

	/// Cancel the handler
	pub fn cancel(&self) {
		self.tracker.stop();
		self.token.cancel();
	}

	pub fn is_done(&self) -> bool {
		self.token.0.is_cancelled()
	}
}

pin_project_lite::pin_project! {
	/// A reference to a context
	/// Can either be owned or borrowed
	pub struct ContextRef<'a> {
		#[pin]
		inner: ContextRefInner<'a>,
	}
}

pin_project_lite::pin_project! {
	#[project = ContextRefInnerProj]
	enum ContextRefInner<'a> {
		Owned {
			#[pin] fut: WaitForCancellationFutureOwned,
			tracker: ContextTracker,
		},
		Ref {
			#[pin] fut: WaitForCancellationFuture<'a>,
		},
	}
}

impl std::future::Future for ContextRef<'_> {
	type Output = ();

	fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
		match self.project().inner.project() {
			ContextRefInnerProj::Owned { fut, .. } => fut.poll(cx),
			ContextRefInnerProj::Ref { fut } => fut.poll(cx),
		}
	}
}

impl From<Context> for ContextRef<'_> {
	fn from(ctx: Context) -> Self {
		ContextRef {
			inner: ContextRefInner::Owned {
				fut: ctx.token.cancelled_owned(),
				tracker: ctx.tracker,
			},
		}
	}
}

impl<'a> From<&'a Context> for ContextRef<'a> {
	fn from(ctx: &'a Context) -> Self {
		ContextRef {
			inner: ContextRefInner::Ref {
				fut: ctx.token.cancelled(),
			},
		}
	}
}

pub trait ContextFutExt<Fut> {
	/// Wraps a future with a context, allowing the future to be cancelled when
	/// the context is done
	fn with_context<'a>(self, ctx: impl Into<ContextRef<'a>>) -> FutureWithContext<'a, Fut>
	where
		Self: Sized;
}

impl<F: IntoFuture> ContextFutExt<F::IntoFuture> for F {
	fn with_context<'a>(self, ctx: impl Into<ContextRef<'a>>) -> FutureWithContext<'a, F::IntoFuture>
	where
		F: IntoFuture,
	{
		FutureWithContext {
			future: self.into_future(),
			ctx: ctx.into(),
			_marker: std::marker::PhantomData,
		}
	}
}

pub trait ContextStreamExt<Stream> {
	/// Wraps a stream with a context, allowing the stream to be stopped when
	/// the context is done
	fn with_context<'a>(self, ctx: impl Into<ContextRef<'a>>) -> StreamWithContext<'a, Stream>
	where
		Self: Sized;
}

impl<F: Stream> ContextStreamExt<F> for F {
	fn with_context<'a>(self, ctx: impl Into<ContextRef<'a>>) -> StreamWithContext<'a, F> {
		StreamWithContext {
			stream: self,
			ctx: ctx.into(),
			_marker: std::marker::PhantomData,
		}
	}
}

pin_project_lite::pin_project! {
	/// A future with a context attached to it.
	///
	/// This future will be cancelled when the context is done.
	pub struct FutureWithContext<'a, F> {
		#[pin]
		future: F,
		#[pin]
		ctx: ContextRef<'a>,
		_marker: std::marker::PhantomData<&'a ()>,
	}
}

impl<F: Future> Future for FutureWithContext<'_, F> {
	type Output = Option<F::Output>;

	fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
		let this = self.project();

		match (this.ctx.poll(cx), this.future.poll(cx)) {
			(_, Poll::Ready(v)) => std::task::Poll::Ready(Some(v)),
			(Poll::Ready(_), Poll::Pending) => std::task::Poll::Ready(None),
			_ => std::task::Poll::Pending,
		}
	}
}

pin_project_lite::pin_project! {
	/// A stream with a context attached to it.
	///
	/// This stream will be cancelled when the context is done.
	pub struct StreamWithContext<'a, F> {
		#[pin]
		stream: F,
		#[pin]
		ctx: ContextRef<'a>,
		_marker: std::marker::PhantomData<&'a ()>,
	}
}

impl<F: Stream> Stream for StreamWithContext<'_, F> {
	type Item = F::Item;

	fn poll_next(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Option<Self::Item>> {
		let this = self.project();

		match (this.ctx.poll(cx), this.stream.poll_next(cx)) {
			(_, Poll::Ready(v)) => std::task::Poll::Ready(v),
			(Poll::Ready(_), Poll::Pending) => std::task::Poll::Ready(None),
			_ => std::task::Poll::Pending,
		}
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		self.stream.size_hint()
	}
}
