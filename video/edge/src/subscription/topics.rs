use std::collections::{HashMap, HashSet, VecDeque};

use tokio::sync::broadcast;

use super::{SubscriptionResponse, TopicKey};

struct Topic {
	tx: broadcast::Sender<SubscriptionResponse>,
	last_value: Option<SubscriptionResponse>,
}

struct ExpiredTopic {
	key: TopicKey,
	expires_at: tokio::time::Instant,
}

#[derive(Default)]
pub struct TopicMap {
	topics: HashMap<TopicKey, Topic>,
	active_topics: HashSet<TopicKey>,
	expired_topics: VecDeque<ExpiredTopic>,
}

impl TopicMap {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn cleanup(&mut self) -> Vec<TopicKey> {
		let now = tokio::time::Instant::now();

		let mut expired = Vec::new();

		while let Some(topic) = self.expired_topics.front() {
			if self.active_topics.contains(&topic.key) {
				self.expired_topics.pop_front();
				continue;
			}

			if topic.expires_at > now {
				break;
			}

			self.topics.remove(&topic.key);
			let topic = self.expired_topics.pop_front().unwrap();
			expired.push(topic.key);
		}

		expired
	}

	pub fn insert(&mut self, key: TopicKey, tx: broadcast::Sender<SubscriptionResponse>) {
		self.topics.insert(key.clone(), Topic { tx, last_value: None });
		self.active_topics.insert(key);
	}

	pub fn unsubscribe(&mut self, key: &TopicKey) {
		let Some(topic) = self.topics.get(key) else {
			return;
		};

		if topic.tx.receiver_count() == 0 && self.active_topics.remove(key) {
			self.expired_topics.push_back(ExpiredTopic {
				key: key.clone(),
				expires_at: tokio::time::Instant::now() + tokio::time::Duration::from_secs(30),
			});
		}
	}

	pub fn close(&mut self, topic: &TopicKey) {
		self.topics.remove(topic);
		self.active_topics.remove(topic);
	}

	pub fn subscribe(
		&mut self,
		key: TopicKey,
	) -> Option<(Option<SubscriptionResponse>, broadcast::Receiver<SubscriptionResponse>)> {
		let resp = self
			.topics
			.get(&key)
			.map(|topic| (topic.last_value.clone(), topic.tx.subscribe()))?;

		self.active_topics.insert(key);

		Some(resp)
	}

	pub fn send(&mut self, key: &TopicKey, value: SubscriptionResponse) -> bool {
		let Some(topic) = self.topics.get_mut(key) else {
			return false;
		};

		topic.last_value = Some(value.clone());

		topic.tx.send(value).ok();

		true
	}

	pub fn is_empty(&self) -> bool {
		self.active_topics.is_empty()
	}
}
