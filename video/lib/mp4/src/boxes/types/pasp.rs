use std::io;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;

use crate::boxes::header::BoxHeader;
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// Pixel Aspect Ratio Box
/// ISO/IEC 14496-12:2022(E) - 12.1.4.2
pub struct Pasp {
	pub header: BoxHeader,
	pub h_spacing: u32,
	pub v_spacing: u32,
}

impl Default for Pasp {
	fn default() -> Self {
		Self::new()
	}
}

impl Pasp {
	pub fn new() -> Self {
		Self {
			header: BoxHeader::new(Self::NAME),
			h_spacing: 1,
			v_spacing: 1,
		}
	}
}

impl BoxType for Pasp {
	const NAME: [u8; 4] = *b"pasp";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let h_spacing = reader.read_u32::<BigEndian>()?;
		let v_spacing = reader.read_u32::<BigEndian>()?;

		Ok(Self {
			header,

			h_spacing,
			v_spacing,
		})
	}

	fn primitive_size(&self) -> u64 {
		4 // h_spacing
        + 4 // v_spacing
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		writer.write_u32::<BigEndian>(self.h_spacing)?;
		writer.write_u32::<BigEndian>(self.v_spacing)?;

		Ok(())
	}
}
