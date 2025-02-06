use std::sync::Arc;

use bytes::Bytes;
use tokio::sync::{mpsc, oneshot};

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

/// A request to publish a stream.
#[derive(Debug)]
pub struct PublishRequest {
    /// The app name to publish to.
    pub app_name: Arc<str>,
    /// The stream name to publish to.
    pub stream_name: Arc<str>,
    /// Respond to the request with a boolean indicating success.
    pub response: oneshot::Sender<bool>,
}

pub type PublishProducer = mpsc::Sender<PublishRequest>;
pub type PublishConsumer = mpsc::Receiver<PublishRequest>;

pub type DataProducer = mpsc::Sender<ChannelData>;
pub type DataConsumer = mpsc::Receiver<ChannelData>;
