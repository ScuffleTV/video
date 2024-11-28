use std::pin::Pin;

use futures_util::Future;
use rtmp::{ChannelData, PublishRequest, SessionError};
use tokio::select;
use tokio::sync::mpsc;

pub struct RtmpSession<'a, F> {
	future: Pin<&'a mut F>,
	publish: mpsc::Receiver<PublishRequest>,
	data: mpsc::Receiver<ChannelData>,
}

pub enum Data {
	Data(Option<ChannelData>),
	Closed(bool),
}

impl<'a, F: Future<Output = Result<bool, SessionError>>> RtmpSession<'a, F> {
	pub fn new(future: Pin<&'a mut F>, publish: mpsc::Receiver<PublishRequest>, data: mpsc::Receiver<ChannelData>) -> Self {
		Self { future, publish, data }
	}

	pub async fn publish(&mut self) -> Result<Option<PublishRequest>, SessionError> {
		select! {
			r = self.future.as_mut() => Ok(r.map(|_| None)?),
			publish = self.publish.recv() => Ok(publish),
		}
	}

	pub async fn data(&mut self) -> Result<Data, SessionError> {
		select! {
			r = self.future.as_mut() => Ok(r.map(Data::Closed)?),
			data = self.data.recv() => Ok(Data::Data(data)),
		}
	}
}
