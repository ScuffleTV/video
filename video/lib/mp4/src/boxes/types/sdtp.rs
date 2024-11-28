use std::io;

use byteorder::WriteBytesExt;
use bytes::{Buf, Bytes};

use crate::boxes::header::{BoxHeader, FullBoxHeader};
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// Sample Dependency Type Box
/// ISO/IEC 14496-12:2022(E) 8.6.4
pub struct Sdtp {
	pub header: FullBoxHeader,
	pub entries: Vec<SdtpEntry>,
}

#[derive(Debug, Clone, PartialEq)]
/// Sample Dependency Type Entry
pub struct SdtpEntry {
	pub sample_is_leading: u8,     // 2 bits
	pub sample_depends_on: u8,     // 2 bits
	pub sample_is_depended_on: u8, // 2 bits
	pub sample_has_redundancy: u8, // 2 bits
}

impl BoxType for Sdtp {
	const NAME: [u8; 4] = *b"sdtp";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let header = FullBoxHeader::demux(header, &mut reader)?;

		let mut entries = Vec::new();
		while reader.has_remaining() {
			let byte = reader.get_u8();
			entries.push(SdtpEntry {
				sample_is_leading: (byte & 0b11000000) >> 6,
				sample_depends_on: (byte & 0b00110000) >> 4,
				sample_is_depended_on: (byte & 0b00001100) >> 2,
				sample_has_redundancy: byte & 0b00000011,
			});
		}

		Ok(Self { header, entries })
	}

	fn primitive_size(&self) -> u64 {
		let size = self.header.size();
		// entries
		size + (self.entries.len() as u64)
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.header.mux(writer)?;

		for entry in &self.entries {
			let byte = (entry.sample_is_leading << 6)
				| (entry.sample_depends_on << 4)
				| (entry.sample_is_depended_on << 2)
				| entry.sample_has_redundancy;
			writer.write_u8(byte)?;
		}

		Ok(())
	}

	fn validate(&self) -> io::Result<()> {
		if self.header.version != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "sdtp box version must be 0"));
		}

		if self.header.flags != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "sdtp box flags must be 0"));
		}

		Ok(())
	}
}
