use std::io;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;

use crate::boxes::header::{BoxHeader, FullBoxHeader};
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// Movie Fragment Header Box
/// ISO/IEC 14496-12:2022(E) - 8.8.5
pub struct Mfhd {
	pub header: FullBoxHeader,
	pub sequence_number: u32,
}

impl Mfhd {
	pub fn new(sequence_number: u32) -> Self {
		Self {
			header: FullBoxHeader::new(Self::NAME, 0, 0),
			sequence_number,
		}
	}
}

impl BoxType for Mfhd {
	const NAME: [u8; 4] = *b"mfhd";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let header = FullBoxHeader::demux(header, &mut reader)?;

		let sequence_number = reader.read_u32::<BigEndian>()?;

		Ok(Self { header, sequence_number })
	}

	fn primitive_size(&self) -> u64 {
		self.header.size() + 4 // sequence_number
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.header.mux(writer)?;
		writer.write_u32::<BigEndian>(self.sequence_number)?;

		Ok(())
	}

	fn validate(&self) -> io::Result<()> {
		if self.header.version != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "mfhd box version must be 0"));
		}

		if self.header.flags != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "mfhd box flags must be 0"));
		}

		Ok(())
	}
}
