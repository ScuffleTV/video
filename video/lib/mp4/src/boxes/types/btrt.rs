use std::io;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;

use crate::boxes::header::BoxHeader;
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// BitRate Box
/// ISO/IEC 14496-12:2022(E) - 8.5.2
pub struct Btrt {
	pub header: BoxHeader,
	pub buffer_size_db: u32,
	pub max_bitrate: u32,
	pub avg_bitrate: u32,
}

impl BoxType for Btrt {
	const NAME: [u8; 4] = *b"btrt";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let buffer_size_db = reader.read_u32::<BigEndian>()?;
		let max_bitrate = reader.read_u32::<BigEndian>()?;
		let avg_bitrate = reader.read_u32::<BigEndian>()?;

		Ok(Self {
			header,

			buffer_size_db,
			max_bitrate,
			avg_bitrate,
		})
	}

	fn primitive_size(&self) -> u64 {
		4 // buffer_size_db
        + 4 // max_bitrate
        + 4 // avg_bitrate
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		writer.write_u32::<BigEndian>(self.buffer_size_db)?;
		writer.write_u32::<BigEndian>(self.max_bitrate)?;
		writer.write_u32::<BigEndian>(self.avg_bitrate)?;

		Ok(())
	}
}
