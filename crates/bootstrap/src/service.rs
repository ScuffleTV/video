use std::pin::Pin;
use std::sync::Arc;
use std::task::{ready, Context, Poll};

pub trait Service<Global>: Send + Sync + 'static + Sized {
	fn name(&self) -> Option<&'static str> {
		None
	}

	/// Initialize the service
	fn enabled(&self, global: &Arc<Global>) -> impl std::future::Future<Output = anyhow::Result<bool>> + Send {
		let _ = global;
		std::future::ready(Ok(true))
	}

	fn run(
		self,
		global: Arc<Global>,
		ctx: scuffle_context::Context,
	) -> impl std::future::Future<Output = anyhow::Result<()>> + Send + 'static {
		let _ = global;
		async move {
			ctx.done().await;
			Ok(())
		}
	}
}

impl<G, F, Fut> Service<G> for F
where
	F: FnOnce(Arc<G>, scuffle_context::Context) -> Fut + Send + Sync + 'static,
	Fut: std::future::Future<Output = anyhow::Result<()>> + Send + 'static,
{
	fn run(
		self,
		global: Arc<G>,
		ctx: scuffle_context::Context,
	) -> impl std::future::Future<Output = anyhow::Result<()>> + Send + 'static {
		self(global, ctx)
	}
}

pin_project_lite::pin_project! {
	#[must_use = "futures do nothing unless polled"]
	pub struct NamedFuture<T> {
		name: &'static str,
		#[pin]
		fut: T,
	}
}

impl<T> NamedFuture<T> {
	pub fn new(name: &'static str, fut: T) -> Self {
		Self { name, fut }
	}
}

impl<T> std::future::Future for NamedFuture<T>
where
	T: std::future::Future,
{
	type Output = (&'static str, T::Output);

	fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
		let this = self.project();
		let res = ready!(this.fut.poll(cx));
		Poll::Ready((this.name, res))
	}
}
