use std::io;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;

use crate::boxes::header::{BoxHeader, FullBoxHeader};
use crate::boxes::traits::BoxType;

const OP_COLOR_SIZE: usize = 3;

#[derive(Debug, Clone, PartialEq)]
/// Video Media Header Box
/// ISO/IEC 14496-12:2022(E) - 12.1.2
pub struct Vmhd {
	pub header: FullBoxHeader,

	pub graphics_mode: u16,
	pub opcolor: [u16; OP_COLOR_SIZE],
}

impl Default for Vmhd {
	fn default() -> Self {
		Self::new()
	}
}

impl Vmhd {
	pub fn new() -> Self {
		Self {
			header: FullBoxHeader::new(Self::NAME, 0, 1),
			graphics_mode: 0,
			opcolor: [0; OP_COLOR_SIZE],
		}
	}
}

impl BoxType for Vmhd {
	const NAME: [u8; 4] = *b"vmhd";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let header = FullBoxHeader::demux(header, &mut reader)?;

		let graphics_mode = reader.read_u16::<BigEndian>()?;

		let mut opcolor = [0; OP_COLOR_SIZE];
		for v in opcolor.iter_mut() {
			*v = reader.read_u16::<BigEndian>()?;
		}

		Ok(Self {
			header,
			graphics_mode,
			opcolor,
		})
	}

	fn primitive_size(&self) -> u64 {
		let size = self.header.size();
		let size = size + 2; // graphics_mode
		size + 2 * OP_COLOR_SIZE as u64 // opcolor
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.header.mux(writer)?;

		writer.write_u16::<BigEndian>(self.graphics_mode)?;

		for i in 0..OP_COLOR_SIZE {
			writer.write_u16::<BigEndian>(self.opcolor[i])?;
		}

		Ok(())
	}
}
