use std::io;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;

use crate::boxes::header::{BoxHeader, FullBoxHeader};
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// Composition Time to Sample Box
/// ISO/IEC 14496-12:2022(E) - 8.6.1
pub struct Ctts {
	pub header: FullBoxHeader,
	pub entries: Vec<CttsEntry>,
}

#[derive(Debug, Clone, PartialEq)]
/// Entry in the Composition Time to Sample Box
pub struct CttsEntry {
	pub sample_count: u32,
	pub sample_offset: i64,
}

impl BoxType for Ctts {
	const NAME: [u8; 4] = *b"ctts";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let header = FullBoxHeader::demux(header, &mut reader)?;

		let entry_count = reader.read_u32::<BigEndian>()?;

		let mut entries = Vec::with_capacity(entry_count as usize);

		for _ in 0..entry_count {
			let sample_count = reader.read_u32::<BigEndian>()?;
			let sample_offset = if header.version == 1 {
				reader.read_i32::<BigEndian>()? as i64
			} else {
				reader.read_u32::<BigEndian>()? as i64
			};

			entries.push(CttsEntry {
				sample_count,
				sample_offset,
			});
		}

		Ok(Self { header, entries })
	}

	fn primitive_size(&self) -> u64 {
		self.header.size()
        + 4 // entry_count
        + (self.entries.len() as u64 * 8) // entries
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.header.mux(writer)?;

		writer.write_u32::<BigEndian>(self.entries.len() as u32)?;

		for entry in &self.entries {
			writer.write_u32::<BigEndian>(entry.sample_count)?;

			if self.header.version == 1 {
				writer.write_i32::<BigEndian>(entry.sample_offset as i32)?;
			} else {
				writer.write_u32::<BigEndian>(entry.sample_offset as u32)?;
			}
		}

		Ok(())
	}

	fn validate(&self) -> io::Result<()> {
		if self.header.flags != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "ctts flags must be 0"));
		}

		if self.header.version > 1 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "ctts version must be 0 or 1"));
		}

		Ok(())
	}
}
