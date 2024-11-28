use std::io::{
	Read, {self},
};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;

use crate::boxes::header::BoxHeader;
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// Color Box
/// ISO/IEC 14496-12:2022(E) - 12.1.5
pub struct Colr {
	pub header: BoxHeader,
	pub color_type: ColorType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ColorType {
	Nclx {
		color_primaries: u16,
		transfer_characteristics: u16,
		matrix_coefficients: u16,
		full_range_flag: bool,
	},
	Unknown(([u8; 4], Bytes)),
}

impl Colr {
	pub fn new(color_type: ColorType) -> Self {
		Self {
			header: BoxHeader::new(Self::NAME),
			color_type,
		}
	}
}

impl BoxType for Colr {
	const NAME: [u8; 4] = *b"colr";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let mut color_type = [0; 4];
		reader.read_exact(&mut color_type)?;

		let color_type = match &color_type {
			b"nclx" => {
				let color_primaries = reader.read_u16::<BigEndian>()?;
				let transfer_characteristics = reader.read_u16::<BigEndian>()?;
				let matrix_coefficients = reader.read_u16::<BigEndian>()?;
				let full_range_flag = (reader.read_u8()? >> 7) == 1;

				ColorType::Nclx {
					color_primaries,
					transfer_characteristics,
					matrix_coefficients,
					full_range_flag,
				}
			}
			_ => {
				let pos = reader.position() as usize;
				let data = reader.into_inner().slice(pos..);
				ColorType::Unknown((color_type, data))
			}
		};

		Ok(Self { header, color_type })
	}

	fn primitive_size(&self) -> u64 {
		let size = match &self.color_type {
			ColorType::Nclx { .. } => 2 + 2 + 2 + 1,            // 7 bytes
			ColorType::Unknown((_, data)) => data.len() as u64, // unknown size
		};

		size + 4 // color type
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		match &self.color_type {
			ColorType::Nclx {
				color_primaries,
				transfer_characteristics,
				matrix_coefficients,
				full_range_flag,
			} => {
				writer.write_all(b"nclx")?;
				writer.write_u16::<BigEndian>(*color_primaries)?;
				writer.write_u16::<BigEndian>(*transfer_characteristics)?;
				writer.write_u16::<BigEndian>(*matrix_coefficients)?;
				writer.write_u8(if *full_range_flag { 0x80 } else { 0x00 })?;
			}
			ColorType::Unknown((color_type, data)) => {
				writer.write_all(color_type)?;
				writer.write_all(data)?;
			}
		}

		Ok(())
	}
}
