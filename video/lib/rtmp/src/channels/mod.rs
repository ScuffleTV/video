use bytes::Bytes;
use tokio::sync::{mpsc, oneshot};

pub type UniqueID = uuid::Uuid;

#[derive(Clone, Debug)]
pub enum ChannelData {
	Video { timestamp: u32, data: Bytes },
	Audio { timestamp: u32, data: Bytes },
	Metadata { timestamp: u32, data: Bytes },
}

impl ChannelData {
	pub fn timestamp(&self) -> u32 {
		match self {
			ChannelData::Video { timestamp, .. } => *timestamp,
			ChannelData::Audio { timestamp, .. } => *timestamp,
			ChannelData::Metadata { timestamp, .. } => *timestamp,
		}
	}

	pub fn data(&self) -> &Bytes {
		match self {
			ChannelData::Video { data, .. } => data,
			ChannelData::Audio { data, .. } => data,
			ChannelData::Metadata { data, .. } => data,
		}
	}
}

#[derive(Debug)]
pub struct PublishRequest {
	pub app_name: String,
	pub stream_name: String,
	pub response: oneshot::Sender<UniqueID>,
}

pub type PublishProducer = mpsc::Sender<PublishRequest>;
pub type PublishConsumer = mpsc::Receiver<PublishRequest>;

pub type DataProducer = mpsc::Sender<ChannelData>;
pub type DataConsumer = mpsc::Receiver<ChannelData>;
