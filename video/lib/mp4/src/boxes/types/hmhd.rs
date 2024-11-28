use std::io;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;

use crate::boxes::header::{BoxHeader, FullBoxHeader};
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// Hint Media Header Box
/// ISO/IEC 14496-12:2022(E) - 12.4.3
pub struct Hmhd {
	pub header: FullBoxHeader,
	pub max_pdu_size: u16,
	pub avg_pdu_size: u16,
	pub max_bitrate: u32,
	pub avg_bitrate: u32,
	pub reserved: u32,
}

impl BoxType for Hmhd {
	const NAME: [u8; 4] = *b"hmhd";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let header = FullBoxHeader::demux(header, &mut reader)?;

		let max_pdu_size = reader.read_u16::<BigEndian>()?;
		let avg_pdu_size = reader.read_u16::<BigEndian>()?;
		let max_bitrate = reader.read_u32::<BigEndian>()?;
		let avg_bitrate = reader.read_u32::<BigEndian>()?;
		let reserved = reader.read_u32::<BigEndian>()?;

		Ok(Self {
			header,
			max_pdu_size,
			avg_pdu_size,
			max_bitrate,
			avg_bitrate,
			reserved,
		})
	}

	fn primitive_size(&self) -> u64 {
		self.header.size()
        + 2 // max_pdu_size
        + 2 // avg_pdu_size
        + 4 // max_bitrate
        + 4 // avg_bitrate
        + 4 // reserved
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.header.mux(writer)?;

		writer.write_u16::<BigEndian>(self.max_pdu_size)?;
		writer.write_u16::<BigEndian>(self.avg_pdu_size)?;
		writer.write_u32::<BigEndian>(self.max_bitrate)?;
		writer.write_u32::<BigEndian>(self.avg_bitrate)?;
		writer.write_u32::<BigEndian>(self.reserved)?;

		Ok(())
	}

	fn validate(&self) -> io::Result<()> {
		if self.header.version != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "hmhd version must be 0"));
		}

		if self.header.flags != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "hmhd flags must be 0"));
		}

		if self.reserved != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "hmhd reserved must be 0"));
		}

		Ok(())
	}
}
