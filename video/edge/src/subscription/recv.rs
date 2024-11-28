use tokio::sync::broadcast;

use super::{Event, SubscriptionManager, SubscriptionResponse, TopicKey};

pub struct SubscriberReceiver<'a> {
	key: TopicKey,
	last_value: Option<SubscriptionResponse>,
	rx: broadcast::Receiver<SubscriptionResponse>,
	manager: &'a SubscriptionManager,
}

impl<'a> SubscriberReceiver<'a> {
	pub fn new(
		key: TopicKey,
		last_value: Option<SubscriptionResponse>,
		rx: broadcast::Receiver<SubscriptionResponse>,
		manager: &'a SubscriptionManager,
	) -> Self {
		Self {
			key,
			last_value,
			rx,
			manager,
		}
	}

	pub async fn next(&mut self) -> Option<SubscriptionResponse> {
		if self.last_value.is_some() {
			return self.last_value.take();
		}

		self.rx.recv().await.ok()
	}
}

impl Drop for SubscriberReceiver<'_> {
	fn drop(&mut self) {
		self.manager.events_tx.send(Event::Unsubscribe { key: self.key.clone() }).ok();
	}
}
