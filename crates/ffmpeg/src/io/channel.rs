use std::sync::{Arc, Mutex};

use bytes::{Buf, BytesMut};

/// A wrapper around a channel that implements `std::io::Read` and
/// `std::io::Write`. The wrapper allows for the channel to be used with the
/// `Input` and `Output` structs.
#[derive(Debug, Clone)]
pub struct ChannelCompat<T: Send> {
	/// I am unsure if the mutex is needed here. I do not think it is, but I am
	/// not sure. FFmpeg might require the IO to be synchronized, but I do not
	/// think it does.
	inner: Arc<Mutex<T>>,
	total: usize,
	pkt_idx: usize,
	buffer: BytesMut,
}

impl<T: Send> ChannelCompat<T> {
	pub fn new(inner: T) -> Self {
		Self {
			inner: Arc::new(Mutex::new(inner)),
			total: 0,
			pkt_idx: 0,
			buffer: BytesMut::new(),
		}
	}
}

pub trait ChannelCompatRecv: Send {
	type Data: AsRef<[u8]>;

	fn channel_recv(&mut self) -> Option<Self::Data>;

	fn into_compat(self) -> ChannelCompat<Self>
	where
		Self: Sized,
	{
		ChannelCompat::new(self)
	}
}

pub trait ChannelCompatSend: Send {
	type Data: From<Vec<u8>>;

	fn channel_send(&mut self, data: Self::Data) -> bool;

	fn into_compat(self) -> ChannelCompat<Self>
	where
		Self: Sized,
	{
		ChannelCompat::new(self)
	}
}

#[cfg(feature = "tokio-channel")]
impl<D: AsRef<[u8]> + Send> ChannelCompatRecv for tokio::sync::mpsc::Receiver<D> {
	type Data = D;

	fn channel_recv(&mut self) -> Option<Self::Data> {
		self.blocking_recv()
	}
}

#[cfg(feature = "tokio-channel")]
impl<D: From<Vec<u8>> + Send> ChannelCompatSend for tokio::sync::mpsc::Sender<D> {
	type Data = D;

	fn channel_send(&mut self, data: Self::Data) -> bool {
		self.blocking_send(data).is_ok()
	}
}

#[cfg(feature = "tokio-channel")]
impl<D: AsRef<[u8]> + Send> ChannelCompatRecv for tokio::sync::mpsc::UnboundedReceiver<D> {
	type Data = D;

	fn channel_recv(&mut self) -> Option<Self::Data> {
		self.blocking_recv()
	}
}

#[cfg(feature = "tokio-channel")]
impl<D: From<Vec<u8>> + Send> ChannelCompatSend for tokio::sync::mpsc::UnboundedSender<D> {
	type Data = D;

	fn channel_send(&mut self, data: Self::Data) -> bool {
		self.send(data).is_ok()
	}
}

#[cfg(feature = "tokio-channel")]
impl<D: AsRef<[u8]> + Clone + Send> ChannelCompatRecv for tokio::sync::broadcast::Receiver<D> {
	type Data = D;

	fn channel_recv(&mut self) -> Option<Self::Data> {
		self.blocking_recv().ok()
	}
}

#[cfg(feature = "tokio-channel")]
impl<D: From<Vec<u8>> + Clone + Send> ChannelCompatSend for tokio::sync::broadcast::Sender<D> {
	type Data = D;

	fn channel_send(&mut self, data: Self::Data) -> bool {
		self.send(data).is_ok()
	}
}

#[cfg(feature = "crossbeam-channel")]
impl<D: AsRef<[u8]> + Send> ChannelCompatRecv for crossbeam_channel::Receiver<D> {
	type Data = D;

	fn channel_recv(&mut self) -> Option<Self::Data> {
		self.recv().ok()
	}
}

#[cfg(feature = "crossbeam-channel")]
impl<D: From<Vec<u8>> + Send> ChannelCompatSend for crossbeam_channel::Sender<D> {
	type Data = D;

	fn channel_send(&mut self, data: Self::Data) -> bool {
		self.send(data).is_ok()
	}
}

impl<D: AsRef<[u8]> + Send> ChannelCompatRecv for std::sync::mpsc::Receiver<D> {
	type Data = D;

	fn channel_recv(&mut self) -> Option<Self::Data> {
		self.recv().ok()
	}
}

impl<D: From<Vec<u8>> + Send> ChannelCompatSend for std::sync::mpsc::Sender<D> {
	type Data = D;

	fn channel_send(&mut self, data: Self::Data) -> bool {
		self.send(data).is_ok()
	}
}

impl<T: ChannelCompatRecv> std::io::Read for ChannelCompat<T> {
	fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
		if self.buffer.is_empty() {
			let data = match self.inner.lock().unwrap().channel_recv() {
				Some(data) => data,
				None => return Ok(0),
			};
			let data = data.as_ref();

			self.pkt_idx += 1;
			self.total += data.len();

			let min = std::cmp::min(buf.len(), data.len());
			buf[..min].copy_from_slice(&data[..min]);
			if min < data.len() {
				self.buffer.extend_from_slice(&data[min..]);
			}
			Ok(min)
		} else {
			let min = std::cmp::min(buf.len(), self.buffer.len());
			buf[..min].copy_from_slice(&self.buffer[..min]);
			self.buffer.advance(min);
			Ok(min)
		}
	}
}

impl<T: ChannelCompatSend> std::io::Write for ChannelCompat<T> {
	fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
		if !self.inner.lock().unwrap().channel_send(buf.to_vec().into()) {
			return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "Unexpected EOF"));
		}

		Ok(buf.len())
	}

	fn flush(&mut self) -> std::io::Result<()> {
		Ok(())
	}
}
