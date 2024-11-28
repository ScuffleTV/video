use std::io;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;

use crate::boxes::header::{BoxHeader, FullBoxHeader};
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// Sample to Group Box
/// ISO/IEC 14496-12:2022(E) - 8.9.2
pub struct Sbgp {
	pub header: FullBoxHeader,
	pub grouping_type: Option<u32>,
	pub entries: Vec<SbgpEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SbgpEntry {
	pub sample_count: u32,
	pub group_description_index: u32,
}

impl BoxType for Sbgp {
	const NAME: [u8; 4] = *b"sbgp";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut data = io::Cursor::new(data);

		let header = FullBoxHeader::demux(header, &mut data)?;

		let grouping_type = if header.version == 1 {
			Some(data.read_u32::<BigEndian>()?)
		} else {
			None
		};
		let entry_count = data.read_u32::<BigEndian>()?;

		let mut entries = Vec::with_capacity(entry_count as usize);

		for _ in 0..entry_count {
			let sample_count = data.read_u32::<BigEndian>()?;
			let group_description_index = data.read_u32::<BigEndian>()?;

			entries.push(SbgpEntry {
				sample_count,
				group_description_index,
			});
		}

		Ok(Self {
			header,
			grouping_type,
			entries,
		})
	}

	fn primitive_size(&self) -> u64 {
		self.header.size()
        + 4  // grouping_type
        + 4 // entry_count
        + (self.entries.len() as u64 * 8) // entries
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.header.mux(writer)?;

		if let Some(grouping_type) = self.grouping_type {
			writer.write_u32::<BigEndian>(grouping_type)?;
		}

		writer.write_u32::<BigEndian>(self.entries.len() as u32)?;

		for entry in &self.entries {
			writer.write_u32::<BigEndian>(entry.sample_count)?;
			writer.write_u32::<BigEndian>(entry.group_description_index)?;
		}

		Ok(())
	}

	fn validate(&self) -> io::Result<()> {
		if self.header.version > 1 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "sbgp box version must be 0 or 1"));
		}

		if self.header.flags != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "sbgp box flags must be 0"));
		}

		if self.header.version == 1 && self.grouping_type.is_none() {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"sbgp box grouping_type must be present when version is 1",
			));
		} else if self.header.version == 0 && self.grouping_type.is_some() {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"sbgp box grouping_type must not be present when version is 0",
			));
		}

		Ok(())
	}
}
