use std::io;

#[derive(Default, Clone, Debug)]
pub struct BitWriter {
	data: Vec<u8>,
	bit_pos: usize,
}

impl BitWriter {
	pub fn write_bit(&mut self, bit: bool) -> io::Result<()> {
		let byte_pos = self.bit_pos / 8;
		let bit_pos = self.bit_pos % 8;

		if byte_pos >= self.data.len() {
			self.data.push(0);
		}

		let byte = &mut self.data[byte_pos];
		if bit {
			*byte |= 1 << (7 - bit_pos);
		} else {
			*byte &= !(1 << (7 - bit_pos));
		}

		self.bit_pos += 1;

		Ok(())
	}

	pub fn write_bits(&mut self, bits: u64, count: usize) -> io::Result<()> {
		for i in 0..count {
			let bit = (bits >> (count - i - 1)) & 1 == 1;
			self.write_bit(bit)?;
		}

		Ok(())
	}

	pub fn into_inner(self) -> Vec<u8> {
		self.data
	}

	pub fn get_ref(&self) -> &Vec<u8> {
		&self.data
	}

	pub fn get_mut(&mut self) -> &mut Vec<u8> {
		&mut self.data
	}

	pub fn get_bit_pos(&self) -> usize {
		self.bit_pos
	}

	pub fn is_aligned(&self) -> bool {
		self.bit_pos % 8 == 0
	}

	pub fn align(&mut self) -> io::Result<()> {
		if !self.is_aligned() {
			self.write_bits(0, 8 - (self.bit_pos % 8))?;
		}

		Ok(())
	}

	pub fn seek_bits(&mut self, count: i64) {
		if count < 0 {
			if self.bit_pos < (-count) as usize {
				self.bit_pos = 0;
			} else {
				self.bit_pos -= (-count) as usize;
			}
		} else if self.bit_pos + count as usize >= self.data.len() * 8 {
			self.bit_pos = self.data.len() * 8;
		} else {
			self.bit_pos += count as usize;
		}
	}

	pub fn seek_to(&mut self, pos: usize) {
		if pos >= self.data.len() * 8 {
			self.bit_pos = self.data.len() * 8;
		} else {
			self.bit_pos = pos;
		}
	}
}

impl io::Write for BitWriter {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		for b in buf {
			self.write_bits(*b as u64, 8)?;
		}

		Ok(buf.len())
	}

	fn flush(&mut self) -> io::Result<()> {
		Ok(())
	}
}

impl io::Seek for BitWriter {
	fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
		match pos {
			io::SeekFrom::Start(pos) => self.seek_to((pos * 8) as usize),
			io::SeekFrom::Current(pos) => self.seek_bits(pos * 8),
			io::SeekFrom::End(pos) => self.seek_to((self.data.len() as i64 + pos) as usize * 8),
		};

		Ok(self.bit_pos as u64)
	}
}
