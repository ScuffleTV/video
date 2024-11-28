use std::sync::Arc;

use async_nats::jetstream::kv::Entry;
use scuffle_utils::context::Context;
use tokio::select;
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};
use tokio_stream::{StreamExt, StreamMap, StreamNotifyClose};

pub use self::recv::SubscriberReceiver;
use self::topics::TopicMap;

mod recv;
mod topics;

pub type SubscriptionResponse = Entry;

#[derive(Debug)]
pub enum Event {
	Subscribe {
		key: TopicKey,
		tx: oneshot::Sender<(Option<SubscriptionResponse>, broadcast::Receiver<SubscriptionResponse>)>,
	},
	Unsubscribe {
		key: TopicKey,
	},
}

type TopicKey = Arc<str>;

pub struct SubscriptionManager {
	events_tx: mpsc::UnboundedSender<Event>,
	events_rx: Mutex<mpsc::UnboundedReceiver<Event>>,
}

impl Default for SubscriptionManager {
	fn default() -> Self {
		// Only one value is needed in the channel.
		// This is a way to get around we cannot await in a drop.
		let (events_tx, events_rx) = mpsc::unbounded_channel();

		Self {
			events_rx: Mutex::new(events_rx),
			events_tx,
		}
	}
}

#[derive(thiserror::Error, Debug)]
pub enum SubscriptionError {
	#[error("failed to send event: {0}")]
	SendEvent(#[from] mpsc::error::SendError<Event>),
	#[error("failed to receive event: {0}")]
	RecvEvent(#[from] oneshot::error::RecvError),
	#[error("failed to subscribe to topic: {0}")]
	SubscribeKv(#[from] async_nats::jetstream::kv::WatchError),
	#[error("failed to subscribe to topic: {0}")]
	SubscribeStream(#[from] async_nats::jetstream::kv::WatcherError),
	#[error("failed to subscribe to topic: {0}")]
	SubscribeOb(#[from] async_nats::jetstream::object_store::WatchError),
}

impl SubscriptionManager {
	pub async fn run(
		&self,
		ctx: &Context,
		metadata_store: &async_nats::jetstream::kv::Store,
	) -> Result<(), SubscriptionError> {
		let mut topics = TopicMap::new();
		let mut subs = StreamMap::new();

		let mut events_rx = self.events_rx.lock().await;

		let mut cleanup_interval = tokio::time::interval(tokio::time::Duration::from_secs(30));

		loop {
			select! {
				event = events_rx.recv() => {
					tracing::debug!("received event: {:?}", event);

					match event.unwrap() {
						Event::Subscribe { key, tx } => {
							match topics.subscribe(key.clone()) {
								Some(resp) => {
									tx.send(resp).ok();
								},
								None => {
									let (btx, rx) = broadcast::channel(16);
									if tx.send((None, rx)).is_err() {
										// TODO: Handle error?
										tracing::warn!("failed to send broadcast receiver to subscriber");
										continue;
									}

									tracing::debug!("subscribing to topic: {:?}", key);

									topics.insert(key.clone(), btx);
									let sub = metadata_store.watch_with_history(&key).await?;
									subs.insert(key.clone(), StreamNotifyClose::new(sub));
								}
							};
						}
						Event::Unsubscribe { key } => {
							tracing::debug!("received unsubscribe event for topic: {:?}", key);
							topics.unsubscribe(&key);

							if topics.is_empty() && ctx.is_done() {
								break;
							}
						}
					}
				}
				Some((topic, message)) = subs.next() => {
					match message {
						Some(message) => {
							let message = message?;
							tracing::debug!("received nats message: {:?}", message);

							if !topics.send(&topic, message) {
								tracing::warn!("message received for unsubscribed topic: {:?}", topic)
							}
						},
						None => {
							// nats subscriber closed
							topics.close(&topic);
						}
					}
				}
				_ = cleanup_interval.tick() => {
					topics.cleanup().iter().for_each(|key| {
						subs.remove(key);
					});
				}
			}
		}

		Ok(())
	}

	pub async fn subscribe_kv(&self, topic: impl ToString) -> Result<SubscriberReceiver<'_>, SubscriptionError> {
		let (tx, rx) = oneshot::channel();

		let key: TopicKey = topic.to_string().into();

		self.events_tx.send(Event::Subscribe { key: key.clone(), tx })?;

		let (last_value, rx) = rx.await?;

		Ok(SubscriberReceiver::new(key, last_value, rx, self))
	}
}
