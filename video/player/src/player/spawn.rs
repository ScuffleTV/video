use tokio::sync::mpsc;

use super::inner::PlayerInnerHolder;
use super::runner::Runner;
use crate::player::events;
use crate::tracing_wasm;

pub fn spawn_runner(runner_tx: mpsc::Sender<()>, mut player_rx: mpsc::Receiver<()>, inner: PlayerInnerHolder) {
	let mut runner = Runner::new(inner);

	wasm_bindgen_futures::spawn_local(async move {
		let mut tick = false;
		let mut wakeup_rx = runner.inner().borrow().runner_settings.request_wakeup.subscribe();

		let _interval = gloo_timers::callback::Interval::new(50, {
			let inner = runner.inner().downgrade();
			move || {
				if let Some(inner) = inner.upgrade() {
					inner.borrow_mut().runner_settings.request_wakeup.send(()).ok();
				} else {
					tracing::warn!("runner dropped, but interval still running");
				}
			}
		});

		loop {
			tokio::select! {
				_ = player_rx.recv() => {
					break;
				}
				_ = async {
					if tick {
						// The reason we want to do this is because of the way javascript works.
						// Mirco tasks such as network events are driven even when the macro task queue is paused due to
						// the page being hidden. SO our wakeup_rx is a micro task queue, and when it polls it means that
						// some request somewhere has completed so we can simply drive the player forward.
						// The timeout future is a macro task, so it will poll only when the page is not hidden,
						// so we can use it to drive the player forward when there are no requests to wait for.
						wakeup_rx.recv().await.ok();
					} else {
						tracing_wasm::scope!(runner.inner().borrow());
						runner.drive().await;
					}
				} => {
					tick = !tick;
				}
			}
		}

		events::dispatch!(runner.inner().borrow_mut().events.emit(events::UserEvent::Destroyed));

		runner.shutdown();

		drop(runner_tx);
	});
}
