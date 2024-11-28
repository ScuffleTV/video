use std::io;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;
use fixed::types::extra::U8;
use fixed::FixedI16;

use crate::boxes::header::{BoxHeader, FullBoxHeader};
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// Sound Media Header Box
/// ISO/IEC 14496-12:2022(E) - 12.2.2
pub struct Smhd {
	pub header: FullBoxHeader,
	pub balance: FixedI16<U8>,
	pub reserved: u16,
}

impl Default for Smhd {
	fn default() -> Self {
		Self::new()
	}
}

impl Smhd {
	pub fn new() -> Self {
		Self {
			header: FullBoxHeader::new(Self::NAME, 0, 0),
			balance: FixedI16::<U8>::from_num(0),
			reserved: 0,
		}
	}
}

impl BoxType for Smhd {
	const NAME: [u8; 4] = *b"smhd";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let header = FullBoxHeader::demux(header, &mut reader)?;

		let balance = reader.read_i16::<BigEndian>()?;
		let reserved = reader.read_u16::<BigEndian>()?;

		Ok(Self {
			header,
			balance: FixedI16::from_bits(balance),
			reserved,
		})
	}

	fn primitive_size(&self) -> u64 {
		let size = self.header.size();
		let size = size + 2; // balance
		// reserved
		size + 2
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.header.mux(writer)?;

		writer.write_i16::<BigEndian>(self.balance.to_bits())?;
		writer.write_u16::<BigEndian>(self.reserved)?;

		Ok(())
	}

	fn validate(&self) -> io::Result<()> {
		if self.header.version != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "smhd version must be 0"));
		}

		if self.header.flags != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "smhd flags must be 0"));
		}

		if self.reserved != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "smhd reserved must be 0"));
		}

		Ok(())
	}
}
