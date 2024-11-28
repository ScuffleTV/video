use scuffle_utilsdataloader::DataLoader;

use crate::config::ApiConfig;
use crate::dataloaders;

pub trait ApiState {
	fn access_token_loader(&self) -> &DataLoader<dataloaders::AccessTokenLoader>;
	fn recording_state_loader(&self) -> &DataLoader<dataloaders::RecordingStateLoader>;
	fn room_loader(&self) -> &DataLoader<dataloaders::RoomLoader>;
	fn events_stream(&self) -> &async_nats::jetstream::stream::Stream;
}

pub trait ApiGlobal:
	binary_helper::global::GlobalCtx
	+ binary_helper::global::GlobalConfigProvider<ApiConfig>
	+ binary_helper::global::GlobalNats
	+ binary_helper::global::GlobalDb
	+ binary_helper::global::GlobalConfig
	+ binary_helper::global::GlobalRedis
	+ ApiState
	+ Send
	+ Sync
	+ 'static
{
}

impl<T> ApiGlobal for T where
	T: binary_helper::global::GlobalCtx
		+ binary_helper::global::GlobalConfigProvider<ApiConfig>
		+ binary_helper::global::GlobalNats
		+ binary_helper::global::GlobalDb
		+ binary_helper::global::GlobalConfig
		+ binary_helper::global::GlobalRedis
		+ ApiState
		+ Send
		+ Sync
		+ 'static
{
}
