use crate::config::EdgeConfig;
use crate::subscription;

pub trait EdgeState {
	fn metadata_store(&self) -> &async_nats::jetstream::kv::Store;
	fn media_store(&self) -> &async_nats::jetstream::object_store::ObjectStore;
	fn subscriber(&self) -> &subscription::SubscriptionManager;
}

pub trait EdgeGlobal:
	binary_helper::global::GlobalCtx
	+ binary_helper::global::GlobalConfigProvider<EdgeConfig>
	+ binary_helper::global::GlobalNats
	+ binary_helper::global::GlobalDb
	+ binary_helper::global::GlobalConfig
	+ EdgeState
	+ Send
	+ Sync
	+ 'static
{
}

impl<T> EdgeGlobal for T where
	T: binary_helper::global::GlobalCtx
		+ binary_helper::global::GlobalConfigProvider<EdgeConfig>
		+ binary_helper::global::GlobalNats
		+ binary_helper::global::GlobalDb
		+ binary_helper::global::GlobalConfig
		+ EdgeState
		+ Send
		+ Sync
		+ 'static
{
}
