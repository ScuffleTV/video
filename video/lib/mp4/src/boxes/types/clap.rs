use std::io;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;

use crate::boxes::header::BoxHeader;
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// Clean Aperture Box
/// ISO/IEC 14496-12:2022(E) - 12.1.4.2
pub struct Clap {
	pub header: BoxHeader,
	pub clean_aperture_width_n: u32,
	pub clean_aperture_width_d: u32,
	pub clean_aperture_height_n: u32,
	pub clean_aperture_height_d: u32,
	pub horiz_off_n: u32,
	pub horiz_off_d: u32,
	pub vert_off_n: u32,
	pub vert_off_d: u32,
}

impl BoxType for Clap {
	const NAME: [u8; 4] = *b"clap";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let clean_aperture_width_n = reader.read_u32::<BigEndian>()?;
		let clean_aperture_width_d = reader.read_u32::<BigEndian>()?;
		let clean_aperture_height_n = reader.read_u32::<BigEndian>()?;
		let clean_aperture_height_d = reader.read_u32::<BigEndian>()?;
		let horiz_off_n = reader.read_u32::<BigEndian>()?;
		let horiz_off_d = reader.read_u32::<BigEndian>()?;
		let vert_off_n = reader.read_u32::<BigEndian>()?;
		let vert_off_d = reader.read_u32::<BigEndian>()?;

		Ok(Self {
			header,

			clean_aperture_width_n,
			clean_aperture_width_d,
			clean_aperture_height_n,
			clean_aperture_height_d,
			horiz_off_n,
			horiz_off_d,
			vert_off_n,
			vert_off_d,
		})
	}

	fn primitive_size(&self) -> u64 {
		4 // clean_aperture_width_n
        + 4 // clean_aperture_width_d
        + 4 // clean_aperture_height_n
        + 4 // clean_aperture_height_d
        + 4 // horiz_off_n
        + 4 // horiz_off_d
        + 4 // vert_off_n
        + 4 // vert_off_d
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		writer.write_u32::<BigEndian>(self.clean_aperture_width_n)?;
		writer.write_u32::<BigEndian>(self.clean_aperture_width_d)?;
		writer.write_u32::<BigEndian>(self.clean_aperture_height_n)?;
		writer.write_u32::<BigEndian>(self.clean_aperture_height_d)?;
		writer.write_u32::<BigEndian>(self.horiz_off_n)?;
		writer.write_u32::<BigEndian>(self.horiz_off_d)?;
		writer.write_u32::<BigEndian>(self.vert_off_n)?;
		writer.write_u32::<BigEndian>(self.vert_off_d)?;

		Ok(())
	}
}
