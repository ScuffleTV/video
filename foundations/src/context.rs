use std::future::{Future, IntoFuture};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::Arc;
use std::task::Poll;

use futures::Stream;
use tokio_util::sync::{CancellationToken, WaitForCancellationFuture, WaitForCancellationFutureOwned};

#[derive(Debug)]
struct ContextTracker(Arc<ContextTrackerInner>);

impl Drop for ContextTracker {
	fn drop(&mut self) {
		if self.active_count.fetch_sub(1, std::sync::atomic::Ordering::Relaxed) == 1
			&& self.stopped.load(std::sync::atomic::Ordering::Relaxed)
		{
			self.notify.notify_waiters();
		}
	}
}

impl Clone for ContextTracker {
	fn clone(&self) -> Self {
		self.active_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
		Self(self.0.clone())
	}
}

impl std::ops::Deref for ContextTracker {
	type Target = ContextTrackerInner;

	fn deref(&self) -> &Self::Target {
		&self.0
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

#[derive(Clone, Debug)]
pub struct Context {
	token: CancellationToken,
	_tracker: ContextTracker,
}

impl Context {
	#[must_use]
	pub fn new() -> (Self, Handler) {
		Handler::global().new_child()
	}

	#[must_use]
	pub fn new_child(&self) -> (Self, Handler) {
		let token = self.token.child_token();
		let tracker = ContextTrackerInner::new();

		(
			Self {
				_tracker: tracker.child(),
				token: token.clone(),
			},
			Handler {
				_token: TokenDropGuard(token),
				tracker,
			},
		)
	}

	#[must_use]
	pub fn global() -> Self {
		Handler::global().context()
	}

	pub async fn done(&self) {
		self.token.cancelled().await;
	}

	pub async fn into_done(self) {
		self.done().await;
	}

	#[must_use]
	pub fn is_done(&self) -> bool {
		self.token.is_cancelled()
	}
}

#[derive(Debug, Clone)]
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
	_token: TokenDropGuard,
	tracker: Arc<ContextTrackerInner>,
}

impl Default for Handler {
	fn default() -> Self {
		Self::new()
	}
}

impl Handler {
	#[must_use]
	pub fn new() -> Handler {
		let token = CancellationToken::new();
		let tracker = ContextTrackerInner::new();

		Handler {
			_token: TokenDropGuard(token),
			tracker,
		}
	}

	#[must_use]
	pub fn global() -> &'static Self {
		static GLOBAL: once_cell::sync::Lazy<Handler> = once_cell::sync::Lazy::new(Handler::new);
		&GLOBAL
	}

	pub async fn shutdown(&self) {
		self.cancel();
		self.done().await;
	}

	pub async fn done(&self) {
		self._token.0.cancelled().await;
		self.tracker.wait().await;
	}

	#[must_use]
	pub fn context(&self) -> Context {
		Context {
			token: self._token.child(),
			_tracker: self.tracker.child(),
		}
	}

	#[must_use]
	pub fn new_child(&self) -> (Context, Handler) {
		self.context().new_child()
	}

	pub fn cancel(&self) {
		self.tracker.stop();
		self._token.cancel();
	}
}

#[pin_project::pin_project(project = ContextRefProj)]
pub enum ContextRef<'a> {
	#[allow(private_interfaces)]
	Owned(#[pin] WaitForCancellationFutureOwned, ContextTracker),
	Ref(#[pin] WaitForCancellationFuture<'a>),
}

impl ContextRef<'_> {
	pub fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<()> {
		match self.project() {
			ContextRefProj::Owned(fut, _) => fut.poll(cx),
			ContextRefProj::Ref(fut) => fut.poll(cx),
		}
	}
}

impl From<Context> for ContextRef<'_> {
	fn from(ctx: Context) -> Self {
		ContextRef::Owned(ctx.token.cancelled_owned(), ctx._tracker)
	}
}

impl<'a> From<&'a Context> for ContextRef<'a> {
	fn from(ctx: &'a Context) -> Self {
		ContextRef::Ref(ctx.token.cancelled())
	}
}

pub trait ContextFutExt<Fut> {
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
		}
	}
}

pub trait ContextStreamExt<Stream> {
	fn with_context<'a>(self, ctx: impl Into<ContextRef<'a>>) -> StreamWithContext<'a, Stream>
	where
		Self: Sized;
}

impl<F: Stream> ContextStreamExt<F> for F {
	fn with_context<'a>(self, ctx: impl Into<ContextRef<'a>>) -> StreamWithContext<'a, F> {
		StreamWithContext {
			stream: self,
			ctx: ctx.into(),
		}
	}
}

#[pin_project::pin_project]
pub struct FutureWithContext<'a, F> {
	#[pin]
	future: F,
	#[pin]
	ctx: ContextRef<'a>,
}

impl<F: Future> Future for FutureWithContext<'_, F> {
	type Output = Option<F::Output>;

	fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
		let mut this = self.as_mut().project();

		match (this.ctx.as_mut().poll(cx), this.future.poll(cx)) {
			(_, Poll::Ready(v)) => std::task::Poll::Ready(Some(v)),
			(Poll::Ready(_), Poll::Pending) => std::task::Poll::Ready(None),
			_ => std::task::Poll::Pending,
		}
	}
}

#[pin_project::pin_project]
pub struct StreamWithContext<'a, F> {
	#[pin]
	stream: F,
	#[pin]
	ctx: ContextRef<'a>,
}

impl<F: Stream> Stream for StreamWithContext<'_, F> {
	type Item = F::Item;

	fn poll_next(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Option<Self::Item>> {
		let mut this = self.as_mut().project();

		match (this.ctx.as_mut().poll(cx), this.stream.poll_next(cx)) {
			(_, Poll::Ready(v)) => std::task::Poll::Ready(v),
			(Poll::Ready(_), Poll::Pending) => std::task::Poll::Ready(None),
			_ => std::task::Poll::Pending,
		}
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		self.stream.size_hint()
	}
}
