use std::io;

use bytes::Bytes;

use crate::boxes::header::{BoxHeader, FullBoxHeader};
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// Null media header box
/// ISO/IEC 14496-12:2022(E) 8.4.5.2
pub struct Nmhd {
	pub header: FullBoxHeader,
}

impl BoxType for Nmhd {
	const NAME: [u8; 4] = *b"nmhd";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let header = FullBoxHeader::demux(header, &mut reader)?;

		Ok(Self { header })
	}

	fn primitive_size(&self) -> u64 {
		self.header.size()
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.header.mux(writer)?;

		Ok(())
	}

	fn validate(&self) -> io::Result<()> {
		if self.header.version != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "nmhd box version must be 0"));
		}

		Ok(())
	}
}
