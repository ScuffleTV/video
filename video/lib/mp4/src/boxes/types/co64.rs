use std::io;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;

use crate::boxes::header::{BoxHeader, FullBoxHeader};
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// Chunk Large Offset Box
/// ISO/IEC 14496-12:2022(E) - 8.7.5
pub struct Co64 {
	pub header: FullBoxHeader,
	pub chunk_offset: Vec<u32>,
}

impl BoxType for Co64 {
	const NAME: [u8; 4] = *b"co64";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let header = FullBoxHeader::demux(header, &mut reader)?;

		let entry_count = reader.read_u32::<BigEndian>()?;
		let mut chunk_offset = Vec::with_capacity(entry_count as usize);
		for _ in 0..entry_count {
			let offset = reader.read_u32::<BigEndian>()?;
			chunk_offset.push(offset);
		}

		Ok(Self { header, chunk_offset })
	}

	fn primitive_size(&self) -> u64 {
		self.header.size()
        + 4 // entry_count
        + (self.chunk_offset.len() as u64 * 4) // chunk_offset
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.header.mux(writer)?;

		writer.write_u32::<BigEndian>(self.chunk_offset.len() as u32)?;
		for offset in &self.chunk_offset {
			writer.write_u32::<BigEndian>(*offset)?;
		}

		Ok(())
	}

	fn validate(&self) -> io::Result<()> {
		if self.header.flags != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "co64 flags must be 0"));
		}

		if self.header.version != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "co64 version must be 0"));
		}

		Ok(())
	}
}
