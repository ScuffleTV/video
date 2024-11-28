use std::io::{
	SeekFrom, {self},
};

use byteorder::ReadBytesExt;
use bytes::{Buf, Bytes};

pub struct BitReader<T: io::Read = io::Cursor<Bytes>> {
	data: T,
	bit_pos: usize,
	current_byte: u8,
}

impl<T: Into<Bytes>> From<T> for BitReader<io::Cursor<Bytes>> {
	fn from(bytes: T) -> Self {
		Self::new(io::Cursor::new(bytes.into()))
	}
}

impl<T: io::Seek + io::Read> BitReader<T> {
	pub fn seek_bits(&mut self, pos: i64) -> io::Result<()> {
		let mut seek_pos = self.data.stream_position()? as i64;
		if !self.is_aligned() && seek_pos > 0 {
			seek_pos -= 1;
		}

		let current_bit_pos = self.bit_pos as i64;
		let new_tb_pos = current_bit_pos + pos + seek_pos * 8;
		if new_tb_pos < 0 {
			return Err(io::Error::new(
				io::ErrorKind::InvalidInput,
				"Cannot seek to a negative position",
			));
		}

		self.seek_to(new_tb_pos as u64)?;

		Ok(())
	}

	pub fn current_byte_bit_pos(&mut self) -> io::Result<u64> {
		let position = self.data.stream_position()?;
		if position == 0 {
			return Ok(0);
		}

		if self.is_aligned() {
			Ok(position * 8)
		} else {
			Ok(position * 8 - 8 + self.bit_pos as u64)
		}
	}

	pub fn seek_to(&mut self, pos: u64) -> io::Result<()> {
		self.data.seek(SeekFrom::Start(pos / 8))?;

		self.bit_pos = (pos % 8) as usize;
		if self.bit_pos != 0 {
			self.current_byte = self.data.read_u8()?;
		}

		Ok(())
	}
}

impl<T: io::Read> BitReader<T> {
	pub fn new(data: T) -> Self {
		Self {
			data,
			bit_pos: 0,
			current_byte: 0,
		}
	}

	pub fn read_bit(&mut self) -> io::Result<bool> {
		if self.is_aligned() {
			self.current_byte = self.data.read_u8()?;
		}

		let bit = (self.current_byte >> (7 - self.bit_pos)) & 1;

		self.bit_pos += 1;
		self.bit_pos %= 8;

		Ok(bit == 1)
	}

	pub fn read_bits(&mut self, count: u8) -> io::Result<u64> {
		let mut bits = 0;
		for _ in 0..count {
			let bit = self.read_bit()?;
			bits <<= 1;
			bits |= bit as u64;
		}

		Ok(bits)
	}

	pub fn into_inner(self) -> T {
		self.data
	}

	pub fn get_ref(&self) -> &T {
		&self.data
	}

	pub fn get_mut(&mut self) -> &mut T {
		&mut self.data
	}

	pub fn get_bit_pos(&self) -> usize {
		self.bit_pos
	}

	pub fn align(&mut self) -> io::Result<()> {
		let amount_to_read = 8 - self.bit_pos;
		self.read_bits(amount_to_read as u8)?;
		Ok(())
	}

	pub fn is_aligned(&self) -> bool {
		self.bit_pos == 0
	}
}

impl<T: io::Read> io::Read for BitReader<T> {
	fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		if self.is_aligned() {
			return self.data.read(buf);
		}

		let mut read = 0;
		for b in buf {
			let mut byte = 0;
			for _ in 0..8 {
				let bit = self.read_bit()?;
				byte <<= 1;
				byte |= bit as u8;
			}
			*b = byte;
			read += 1;
		}

		Ok(read)
	}
}

impl<T: AsRef<[u8]>> BitReader<io::Cursor<T>> {
	pub fn is_empty(&self) -> bool {
		self.data.position() as usize == self.data.get_ref().as_ref().len()
	}

	pub fn remaining_bits(&self) -> usize {
		let remaining = self.data.remaining();

		if self.is_aligned() {
			remaining * 8
		} else {
			remaining * 8 + 8 - self.bit_pos
		}
	}
}

impl<T: io::Seek + io::Read> io::Seek for BitReader<T> {
	fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
		match pos {
			io::SeekFrom::Start(pos) => {
				self.seek_to(pos * 8)?;
			}
			io::SeekFrom::Current(pos) => {
				self.seek_bits(pos * 8)?;
			}
			io::SeekFrom::End(pos) => {
				let end = self.data.seek(io::SeekFrom::End(0))? as i64;
				self.seek_to((end + pos) as u64 * 8)?;
			}
		}

		self.data.stream_position()
	}
}
