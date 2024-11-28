use scuffle_utilsgrpc::TlsSettings;

use crate::config::TranscoderConfig;

pub trait TranscoderState {
	fn metadata_store(&self) -> &async_nats::jetstream::kv::Store;
	fn media_store(&self) -> &async_nats::jetstream::object_store::ObjectStore;
	fn ingest_tls(&self) -> Option<TlsSettings>;
}

pub trait TranscoderGlobal:
	binary_helper::global::GlobalCtx
	+ binary_helper::global::GlobalConfigProvider<TranscoderConfig>
	+ binary_helper::global::GlobalNats
	+ binary_helper::global::GlobalDb
	+ binary_helper::global::GlobalConfig
	+ TranscoderState
	+ Send
	+ Sync
	+ 'static
{
}

impl<T> TranscoderGlobal for T where
	T: binary_helper::global::GlobalCtx
		+ binary_helper::global::GlobalConfigProvider<TranscoderConfig>
		+ binary_helper::global::GlobalNats
		+ binary_helper::global::GlobalDb
		+ binary_helper::global::GlobalConfig
		+ TranscoderState
		+ Send
		+ Sync
		+ 'static
{
}
