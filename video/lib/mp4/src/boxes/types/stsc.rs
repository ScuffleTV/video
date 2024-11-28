use std::io;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;

use crate::boxes::header::{BoxHeader, FullBoxHeader};
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// Sample To Chunk Box
/// ISO/IEC 14496-12:2022(E) - 8.7.4
pub struct Stsc {
	pub header: FullBoxHeader,
	pub entries: Vec<StscEntry>,
}

impl Stsc {
	pub fn new(entries: Vec<StscEntry>) -> Self {
		Self {
			header: FullBoxHeader::new(Self::NAME, 0, 0),
			entries,
		}
	}
}

#[derive(Debug, Clone, PartialEq)]
/// Sample To Chunk Entry
pub struct StscEntry {
	pub first_chunk: u32,
	pub samples_per_chunk: u32,
	pub sample_description_index: u32,
}

impl BoxType for Stsc {
	const NAME: [u8; 4] = *b"stsc";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let header = FullBoxHeader::demux(header, &mut reader)?;

		let entry_count = reader.read_u32::<BigEndian>()?;
		let mut entries = Vec::with_capacity(entry_count as usize);
		for _ in 0..entry_count {
			let first_chunk = reader.read_u32::<BigEndian>()?;
			let samples_per_chunk = reader.read_u32::<BigEndian>()?;
			let sample_description_index = reader.read_u32::<BigEndian>()?;

			entries.push(StscEntry {
				first_chunk,
				samples_per_chunk,
				sample_description_index,
			});
		}

		Ok(Self { header, entries })
	}

	fn primitive_size(&self) -> u64 {
		let size = self.header.size();
		let size = size + 4; // entry_count
		// entries
		size + (self.entries.len() as u64 * 12)
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.header.mux(writer)?;

		writer.write_u32::<BigEndian>(self.entries.len() as u32)?;
		for entry in &self.entries {
			writer.write_u32::<BigEndian>(entry.first_chunk)?;
			writer.write_u32::<BigEndian>(entry.samples_per_chunk)?;
			writer.write_u32::<BigEndian>(entry.sample_description_index)?;
		}

		Ok(())
	}

	fn validate(&self) -> io::Result<()> {
		if self.header.version != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "stsc box version must be 0"));
		}

		if self.header.flags != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "stsc box flags must be 0"));
		}

		Ok(())
	}
}
