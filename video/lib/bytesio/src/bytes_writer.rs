use std::io;

use bytes::{Bytes, BytesMut};

#[derive(Default)]
pub struct BytesWriter {
	bytes: Vec<u8>,
}

impl BytesWriter {
	pub fn extract_current_bytes(&mut self) -> BytesMut {
		let mut rv_data = BytesMut::new();
		rv_data.extend_from_slice(&self.bytes.clone()[..]);
		self.bytes.clear();

		rv_data
	}

	pub fn get_current_bytes(&mut self) -> BytesMut {
		let mut rv_data = BytesMut::new();
		rv_data.extend_from_slice(&self.bytes[..]);

		rv_data
	}

	pub fn dispose(self) -> Bytes {
		self.bytes.into()
	}
}

impl io::Write for BytesWriter {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		self.bytes.write(buf)
	}

	fn flush(&mut self) -> io::Result<()> {
		self.bytes.flush()
	}
}
