use std::io;

use bytes::{Bytes, BytesMut};

pub struct BytesReader {
	buffer: BytesMut,
}

impl BytesReader {
	pub fn new(buffer: BytesMut) -> Self {
		Self { buffer }
	}

	pub fn extend_from_slice(&mut self, extend: &[u8]) {
		self.buffer.extend_from_slice(extend)
	}

	pub fn read_bytes(&mut self, bytes_num: usize) -> io::Result<BytesMut> {
		if self.buffer.len() < bytes_num {
			Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Not enough bytes"))
		} else {
			Ok(self.buffer.split_to(bytes_num))
		}
	}

	pub fn advance_bytes(&'_ self, bytes_num: usize) -> io::Result<&'_ [u8]> {
		if self.buffer.len() < bytes_num {
			Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Not enough bytes"))
		} else {
			Ok(self.buffer[..bytes_num].as_ref())
		}
	}

	pub fn advance_bytes_cursor(&'_ self, bytes_num: usize) -> io::Result<io::Cursor<&'_ [u8]>> {
		Ok(io::Cursor::new(self.advance_bytes(bytes_num)?))
	}

	pub fn get(&self, index: usize) -> io::Result<u8> {
		if index >= self.len() {
			Err(io::Error::new(io::ErrorKind::UnexpectedEof, "Not enough bytes"))
		} else {
			Ok(*self.buffer.get(index).unwrap())
		}
	}

	pub fn len(&self) -> usize {
		self.buffer.len()
	}

	pub fn is_empty(&self) -> bool {
		self.buffer.is_empty()
	}

	pub fn extract_remaining_bytes(&mut self) -> BytesMut {
		self.buffer.split_to(self.buffer.len())
	}

	pub fn get_remaining_bytes(&self) -> BytesMut {
		self.buffer.clone()
	}
}

impl io::Read for BytesReader {
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		let amount = std::cmp::min(buf.len(), self.buffer.len());
		let remaining = self.read_bytes(amount)?;
		buf[..amount].copy_from_slice(&remaining[..amount]);
		Ok(amount)
	}
}

pub trait BytesCursor {
	fn extract_remaining(&mut self) -> Bytes;
	fn read_slice(&mut self, size: usize) -> io::Result<Bytes>;
}

impl BytesCursor for io::Cursor<Bytes> {
	fn extract_remaining(&mut self) -> Bytes {
		let position = self.position() as usize;
		let remaining = self.get_ref().slice(position..);
		self.set_position(self.get_ref().len() as u64);
		remaining
	}

	fn read_slice(&mut self, size: usize) -> io::Result<Bytes> {
		let position = self.position() as usize;
		if position + size > self.get_ref().len() {
			return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "not enough bytes"));
		}

		let slice = self.get_ref().slice(position..position + size);
		self.set_position((position + size) as u64);

		Ok(slice)
	}
}
