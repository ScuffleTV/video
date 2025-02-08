//! A crate designed to provide the ability to cancel futures using a context
//! go-like approach, allowing for graceful shutdowns and cancellations.
//!
//! ## Why do we need this?
//!
//! Its often useful to wait for all the futures to shutdown or to cancel them
//! when we no longer care about the results. This crate provides an interface
//! to cancel all futures associated with a context or wait for them to finish
//! before shutting down. Allowing for graceful shutdowns and cancellations.
//!
//! ## Usage
//!
//! Here is an example of how to use the `Context` to cancel a spawned task.
//!
//! ```rust
//! # use scuffle_context::{Context, ContextFutExt};
//! # tokio_test::block_on(async {
//! let (ctx, handler) = Context::new();
//!
//! tokio::spawn(async {
//!     // Do some work
//!     tokio::time::sleep(std::time::Duration::from_secs(10)).await;
//! }.with_context(ctx));
//!
//! // Will stop the spawned task and cancel all associated futures.
//! handler.cancel();
//! # });
//! ```
//!
//! ## License
//!
//! This project is licensed under the [MIT](./LICENSE.MIT) or
//! [Apache-2.0](./LICENSE.Apache-2.0) license. You can choose between one of
//! them if you use this work.
//!
//! `SPDX-License-Identifier: MIT OR Apache-2.0`
#![cfg_attr(all(coverage_nightly, test), feature(coverage_attribute))]

use std::sync::atomic::{AtomicBool, AtomicUsize};
use std::sync::Arc;

use tokio_util::sync::CancellationToken;

/// For extending types.
mod ext;

pub use ext::*;

/// Create by calling [`ContextTrackerInner::child`].
#[derive(Debug)]
struct ContextTracker(Arc<ContextTrackerInner>);

impl Drop for ContextTracker {
    fn drop(&mut self) {
        let prev_active_count = self.0.active_count.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        // If this was the last active `ContextTracker` and the context has been
        // stopped, then notify the waiters
        if prev_active_count == 1 && self.0.stopped.load(std::sync::atomic::Ordering::Relaxed) {
            self.0.notify.notify_waiters();
        }
    }
}

#[derive(Debug)]
struct ContextTrackerInner {
    stopped: AtomicBool,
    /// This count keeps track of the number of `ContextTrackers` that exist for
    /// this `ContextTrackerInner`.
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

    /// Create a new `ContextTracker` from an `Arc<ContextTrackerInner>`.
    fn child(self: &Arc<Self>) -> ContextTracker {
        self.active_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        ContextTracker(Arc::clone(self))
    }

    /// Mark this `ContextTrackerInner` as stopped.
    fn stop(&self) {
        self.stopped.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Wait for this `ContextTrackerInner` to be stopped and all associated
    /// `ContextTracker`s to be dropped.
    async fn wait(&self) {
        let notify = self.notify.notified();

        // If there are no active children, then the notify will never be called
        if self.active_count.load(std::sync::atomic::Ordering::Relaxed) == 0 {
            return;
        }

        notify.await;
    }
}

/// A context for cancelling futures and waiting for shutdown.
///
/// A context can be created from a handler by calling [`Handler::context`] or
/// from another context by calling [`Context::new_child`] so to have a
/// hierarchy of contexts.
///
/// Contexts can then be attached to futures or streams in order to
/// automatically cancel them when the context is done, when invoking
/// [`Handler::cancel`].
/// The [`Handler::shutdown`] method will block until all contexts have been
/// dropped allowing for a graceful shutdown.
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

impl Default for Context {
    fn default() -> Self {
        Self::global()
    }
}

impl Context {
    #[must_use]
    /// Create a new context using the global handler.
    /// Returns a child context and child handler of the global handler.
    pub fn new() -> (Self, Handler) {
        Handler::global().new_child()
    }

    #[must_use]
    /// Create a new child context from this context.
    /// Returns a new child context and child handler of this context.
    ///
    /// # Example
    ///
    /// ```rust
    /// use scuffle_context::Context;
    ///
    /// let (parent, parent_handler) = Context::new();
    /// let (child, child_handler) = parent.new_child();
    /// ```
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

    /// Wait for the context to be done (the handler to be shutdown).
    pub async fn done(&self) {
        self.token.cancelled().await;
    }

    /// The same as [`Context::done`] but takes ownership of the context.
    pub async fn into_done(self) {
        self.done().await;
    }

    /// Returns true if the context is done.
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.token.is_cancelled()
    }
}

/// A wrapper type around [`CancellationToken`] that will cancel the token as
/// soon as it is dropped.
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
    /// Create a new handler.
    pub fn new() -> Handler {
        let token = CancellationToken::new();
        let tracker = ContextTrackerInner::new();

        Handler {
            token: Arc::new(TokenDropGuard(token)),
            tracker,
        }
    }

    #[must_use]
    /// Returns the global handler.
    pub fn global() -> &'static Self {
        static GLOBAL: std::sync::OnceLock<Handler> = std::sync::OnceLock::new();

        GLOBAL.get_or_init(Handler::new)
    }

    /// Shutdown the handler and wait for all contexts to be done.
    pub async fn shutdown(&self) {
        self.cancel();
        self.done().await;
    }

    /// Waits for the handler to be done (waiting for all contexts to be done).
    pub async fn done(&self) {
        self.token.0.cancelled().await;
        self.wait().await;
    }

    /// Waits for the handler to be done (waiting for all contexts to be done).
    /// Returns once all contexts are done, even if the handler is not done and
    /// contexts can be created after this call.
    pub async fn wait(&self) {
        self.tracker.wait().await;
    }

    #[must_use]
    /// Create a new context from this handler.
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

    /// Cancel the handler.
    pub fn cancel(&self) {
        self.tracker.stop();
        self.token.cancel();
    }

    /// Returns true if the handler is done.
    pub fn is_done(&self) -> bool {
        self.token.0.is_cancelled()
    }
}

#[cfg_attr(all(coverage_nightly, test), coverage(off))]
#[cfg(test)]
mod tests {
    use scuffle_future_ext::FutureExt;

    use crate::{Context, Handler};

    #[tokio::test]
    async fn new() {
        let (ctx, handler) = Context::new();
        assert!(!handler.is_done());
        assert!(!ctx.is_done());

        let handler = Handler::default();
        assert!(!handler.is_done());
    }

    #[tokio::test]
    async fn cancel() {
        let (ctx, handler) = Context::new();
        let (child_ctx, child_handler) = ctx.new_child();
        let child_ctx2 = ctx.clone();

        assert!(!handler.is_done());
        assert!(!ctx.is_done());
        assert!(!child_handler.is_done());
        assert!(!child_ctx.is_done());
        assert!(!child_ctx2.is_done());

        handler.cancel();

        assert!(handler.is_done());
        assert!(ctx.is_done());
        assert!(child_handler.is_done());
        assert!(child_ctx.is_done());
        assert!(child_ctx2.is_done());
    }

    #[tokio::test]
    async fn cancel_child() {
        let (ctx, handler) = Context::new();
        let (child_ctx, child_handler) = ctx.new_child();

        assert!(!handler.is_done());
        assert!(!ctx.is_done());
        assert!(!child_handler.is_done());
        assert!(!child_ctx.is_done());

        child_handler.cancel();

        assert!(!handler.is_done());
        assert!(!ctx.is_done());
        assert!(child_handler.is_done());
        assert!(child_ctx.is_done());
    }

    #[tokio::test]
    async fn shutdown() {
        let (ctx, handler) = Context::new();

        assert!(!handler.is_done());
        assert!(!ctx.is_done());

        // This is expected to timeout
        assert!(handler
            .shutdown()
            .with_timeout(std::time::Duration::from_millis(200))
            .await
            .is_err());
        assert!(handler.is_done());
        assert!(ctx.is_done());
        assert!(ctx
            .into_done()
            .with_timeout(std::time::Duration::from_millis(200))
            .await
            .is_ok());

        assert!(handler
            .shutdown()
            .with_timeout(std::time::Duration::from_millis(200))
            .await
            .is_ok());
        assert!(handler
            .wait()
            .with_timeout(std::time::Duration::from_millis(200))
            .await
            .is_ok());
        assert!(handler
            .done()
            .with_timeout(std::time::Duration::from_millis(200))
            .await
            .is_ok());
        assert!(handler.is_done());
    }

    #[tokio::test]
    async fn global_handler() {
        let handler = Handler::global();

        assert!(!handler.is_done());

        handler.cancel();

        assert!(handler.is_done());
        assert!(Handler::global().is_done());
        assert!(Context::global().is_done());

        let (child_ctx, child_handler) = Handler::global().new_child();
        assert!(child_handler.is_done());
        assert!(child_ctx.is_done());
    }
}
