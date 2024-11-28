use std::time::Duration;

use bytes::{Bytes, BytesMut};
use futures::SinkExt;
use scuffle_utils::prelude::FutureTimeout;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_stream::StreamExt;
use tokio_util::codec::{BytesCodec, Framed};

use super::bytesio_errors::BytesIOError;

pub trait AsyncReadWrite: AsyncRead + AsyncWrite + Unpin + Send + Sync {}

impl<T> AsyncReadWrite for T where T: AsyncRead + AsyncWrite + Unpin + Send + Sync {}

pub struct BytesIO<S: AsyncReadWrite> {
	stream: Framed<S, BytesCodec>,
}

impl<S: AsyncReadWrite> BytesIO<S> {
	pub fn new(stream: S) -> Self {
		Self {
			stream: Framed::new(stream, BytesCodec::new()),
		}
	}

	pub fn with_capacity(stream: S, capacity: usize) -> Self {
		Self {
			stream: Framed::with_capacity(stream, BytesCodec::new(), capacity),
		}
	}

	pub async fn write(&mut self, bytes: Bytes) -> Result<(), BytesIOError> {
		self.stream.send(bytes).await.map_err(|_| BytesIOError::ClientClosed)?;

		Ok(())
	}

	pub async fn read(&mut self) -> Result<BytesMut, BytesIOError> {
		let Some(Ok(message)) = self.stream.next().await else {
			return Err(BytesIOError::ClientClosed);
		};

		Ok(message)
	}

	pub async fn read_timeout(&mut self, timeout: Duration) -> Result<BytesMut, BytesIOError> {
		self.read().timeout(timeout).await?.map_err(|_| BytesIOError::ClientClosed)
	}

	pub async fn write_timeout(&mut self, bytes: Bytes, timeout: Duration) -> Result<(), BytesIOError> {
		self.write(bytes)
			.timeout(timeout)
			.await?
			.map_err(|_| BytesIOError::ClientClosed)?;

		Ok(())
	}
}
