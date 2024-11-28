use std::io;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;

use crate::boxes::header::{BoxHeader, FullBoxHeader};
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// Track Fragment Base Media Decode Time Box
/// ISO/IEC 14496-12:2022(E) - 8.8.12
pub struct Tfdt {
	pub header: FullBoxHeader,
	pub base_media_decode_time: u64,
}

impl Tfdt {
	pub fn new(base_media_decode_time: u64) -> Self {
		let version = if base_media_decode_time > u32::MAX as u64 { 1 } else { 0 };

		Self {
			header: FullBoxHeader::new(Self::NAME, version, 0),
			base_media_decode_time,
		}
	}
}

impl BoxType for Tfdt {
	const NAME: [u8; 4] = *b"tfdt";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let header = FullBoxHeader::demux(header, &mut reader)?;

		let base_media_decode_time = if header.version == 1 {
			reader.read_u64::<BigEndian>()?
		} else {
			reader.read_u32::<BigEndian>()? as u64
		};

		Ok(Self {
			header,
			base_media_decode_time,
		})
	}

	fn primitive_size(&self) -> u64 {
		self.header.size() + if self.header.version == 1 { 8 } else { 4 }
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.header.mux(writer)?;

		if self.header.version == 1 {
			writer.write_u64::<BigEndian>(self.base_media_decode_time)?;
		} else {
			writer.write_u32::<BigEndian>(self.base_media_decode_time as u32)?;
		}

		Ok(())
	}

	fn validate(&self) -> io::Result<()> {
		if self.header.version > 1 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "tfdt version must be 0 or 1"));
		}

		if self.header.flags != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "tfdt flags must be 0"));
		}

		if self.header.version == 0 && self.base_media_decode_time > u32::MAX as u64 {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"tfdt base_data_offset must be less than 2^32",
			));
		}

		Ok(())
	}
}
