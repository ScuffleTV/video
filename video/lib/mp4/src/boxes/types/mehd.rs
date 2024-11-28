use std::io;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;

use crate::boxes::header::{BoxHeader, FullBoxHeader};
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// Movie Extends Header Box
/// ISO/IEC 14496-12:2022(E) - 8.8.2
pub struct Mehd {
	pub header: FullBoxHeader,
	pub fragment_duration: u64,
}

impl BoxType for Mehd {
	const NAME: [u8; 4] = *b"mehd";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let header = FullBoxHeader::demux(header, &mut reader)?;

		let fragment_duration = if header.version == 1 {
			reader.read_u64::<BigEndian>()?
		} else {
			reader.read_u32::<BigEndian>()? as u64
		};

		Ok(Self {
			header,
			fragment_duration,
		})
	}

	fn primitive_size(&self) -> u64 {
		self.header.size()
			+ if self.header.version == 1 {
				8 // fragment_duration
			} else {
				4 // fragment_duration
			}
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.header.mux(writer)?;

		if self.header.version == 1 {
			writer.write_u64::<BigEndian>(self.fragment_duration)?;
		} else {
			writer.write_u32::<BigEndian>(self.fragment_duration as u32)?;
		}

		Ok(())
	}

	fn validate(&self) -> io::Result<()> {
		if self.header.version > 1 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "mehd version must be 0 or 1"));
		}

		if self.header.version == 0 && self.fragment_duration > u32::MAX as u64 {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"mehd fragment_duration must be less than 2^32",
			));
		}

		Ok(())
	}
}
