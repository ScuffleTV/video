use std::io;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;

use crate::boxes::header::{BoxHeader, FullBoxHeader};
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// Shadow Sync Sample Box
/// ISO/IEC 14496-12:2022(E) - 8.6.3
pub struct Stsh {
	pub header: FullBoxHeader,
	pub entries: Vec<StshEntry>,
}

#[derive(Debug, Clone, PartialEq)]
/// Shadow Sync Sample Entry
pub struct StshEntry {
	pub shadowed_sample_count: u32,
	pub sync_sample_number: u32,
}

impl BoxType for Stsh {
	const NAME: [u8; 4] = *b"stsh";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let header = FullBoxHeader::demux(header, &mut reader)?;

		let entry_count = reader.read_u32::<BigEndian>()?;
		let mut entries = Vec::with_capacity(entry_count as usize);
		for _ in 0..entry_count {
			let shadowed_sample_count = reader.read_u32::<BigEndian>()?;
			let sync_sample_number = reader.read_u32::<BigEndian>()?;

			entries.push(StshEntry {
				shadowed_sample_count,
				sync_sample_number,
			});
		}

		Ok(Self { header, entries })
	}

	fn primitive_size(&self) -> u64 {
		let size = self.header.size();
		let size = size + 4; // entry_count
		// entries
		size + (self.entries.len() as u64 * 8)
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.header.mux(writer)?;

		writer.write_u32::<BigEndian>(self.entries.len() as u32)?;
		for entry in &self.entries {
			writer.write_u32::<BigEndian>(entry.shadowed_sample_count)?;
			writer.write_u32::<BigEndian>(entry.sync_sample_number)?;
		}

		Ok(())
	}

	fn validate(&self) -> io::Result<()> {
		if self.header.version != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "stsh version must be 0"));
		}

		if self.header.flags != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "stsh flags must be 0"));
		}

		Ok(())
	}
}
