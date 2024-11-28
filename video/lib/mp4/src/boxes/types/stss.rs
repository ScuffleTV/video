use std::io;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;

use crate::boxes::header::{BoxHeader, FullBoxHeader};
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// Sync Sample Box
/// ISO/IEC 14496-12:2022(E) - 8.6.2
pub struct Stss {
	pub header: FullBoxHeader,
	pub entries: Vec<u32>,
}

impl BoxType for Stss {
	const NAME: [u8; 4] = *b"stss";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let header = FullBoxHeader::demux(header, &mut reader)?;

		let entry_count = reader.read_u32::<BigEndian>()?;
		let mut entries = Vec::with_capacity(entry_count as usize);
		for _ in 0..entry_count {
			let offset = reader.read_u32::<BigEndian>()?;
			entries.push(offset);
		}

		Ok(Self { header, entries })
	}

	fn primitive_size(&self) -> u64 {
		let size = self.header.size();
		let size = size + 4; // entry_count
		// entries
		size + (self.entries.len() as u64 * 4)
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.header.mux(writer)?;

		writer.write_u32::<BigEndian>(self.entries.len() as u32)?;
		for offset in &self.entries {
			writer.write_u32::<BigEndian>(*offset)?;
		}

		Ok(())
	}

	fn validate(&self) -> io::Result<()> {
		if self.header.version != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "stss version must be 0"));
		}

		if self.header.flags != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "stss flags must be 0"));
		}

		Ok(())
	}
}
