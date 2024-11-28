use std::io;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;

use crate::boxes::header::{BoxHeader, FullBoxHeader};
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// Track Extends Box
/// ISO/IEC 14496-12:2022(E) - 8.8.3
pub struct Trex {
	pub header: FullBoxHeader,
	pub track_id: u32,
	pub default_sample_description_index: u32,
	pub default_sample_duration: u32,
	pub default_sample_size: u32,
	pub default_sample_flags: u32,
}

impl Trex {
	pub fn new(track_id: u32) -> Self {
		Self {
			header: FullBoxHeader::new(Self::NAME, 0, 0),
			track_id,
			default_sample_description_index: 1,
			default_sample_duration: 0,
			default_sample_size: 0,
			default_sample_flags: 0,
		}
	}
}

impl BoxType for Trex {
	const NAME: [u8; 4] = *b"trex";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let header = FullBoxHeader::demux(header, &mut reader)?;

		let track_id = reader.read_u32::<BigEndian>()?;
		let default_sample_description_index = reader.read_u32::<BigEndian>()?;
		let default_sample_duration = reader.read_u32::<BigEndian>()?;
		let default_sample_size = reader.read_u32::<BigEndian>()?;
		let default_sample_flags = reader.read_u32::<BigEndian>()?;

		Ok(Self {
			header,

			track_id,
			default_sample_description_index,
			default_sample_duration,
			default_sample_size,
			default_sample_flags,
		})
	}

	fn primitive_size(&self) -> u64 {
		let size = self.header.size();
		let size = size + 4; // track_id
		let size = size + 4; // default_sample_description_index
		let size = size + 4; // default_sample_duration
		let size = size + 4; // default_sample_size
		// default_sample_flags
		size + 4
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.header.mux(writer)?;

		writer.write_u32::<BigEndian>(self.track_id)?;
		writer.write_u32::<BigEndian>(self.default_sample_description_index)?;
		writer.write_u32::<BigEndian>(self.default_sample_duration)?;
		writer.write_u32::<BigEndian>(self.default_sample_size)?;
		writer.write_u32::<BigEndian>(self.default_sample_flags)?;

		Ok(())
	}

	fn validate(&self) -> io::Result<()> {
		if self.header.version != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "trex version must be 0"));
		}

		if self.header.flags != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "trex flags must be 0"));
		}

		Ok(())
	}
}
