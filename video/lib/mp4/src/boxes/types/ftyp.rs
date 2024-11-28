use std::io;

use byteorder::{ReadBytesExt, WriteBytesExt};
use bytes::Bytes;

use crate::boxes::header::BoxHeader;
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// File Type Box
/// ISO/IEC 14496-12:2022(E) - 4.2.3
pub struct Ftyp {
	pub header: BoxHeader,
	pub major_brand: FourCC,
	pub minor_version: u32,
	pub compatible_brands: Vec<FourCC>,
}

impl Ftyp {
	pub fn new(major_brand: FourCC, minor_version: u32, compatible_brands: Vec<FourCC>) -> Self {
		Self {
			header: BoxHeader::new(Self::NAME),
			major_brand,
			minor_version,
			compatible_brands,
		}
	}
}

impl BoxType for Ftyp {
	const NAME: [u8; 4] = *b"ftyp";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let major_brand =
			FourCC::from(TryInto::<[u8; 4]>::try_into(data.slice(0..4).as_ref()).expect("slice is 4 bytes long"));
		let minor_version = data.slice(4..8).as_ref().read_u32::<byteorder::BigEndian>()?;
		let compatible_brands = {
			let mut compatible_brands = Vec::new();
			let mut data = data.slice(8..);
			while data.len() >= 4 {
				compatible_brands.push(FourCC::from(
					TryInto::<[u8; 4]>::try_into(data.slice(0..4).as_ref()).expect("slice is 4 bytes long"),
				));
				data = data.slice(4..);
			}
			compatible_brands
		};

		Ok(Self {
			header,
			major_brand,
			minor_version,
			compatible_brands,
		})
	}

	fn primitive_size(&self) -> u64 {
		4 + 4 + (self.compatible_brands.len() * 4) as u64
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		writer.write_all(&self.major_brand.to_bytes())?;
		writer.write_u32::<byteorder::BigEndian>(self.minor_version)?;
		for compatible_brand in &self.compatible_brands {
			writer.write_all(&compatible_brand.to_bytes())?;
		}
		Ok(())
	}
}

#[derive(Debug, Clone, PartialEq)]
/// FourCC (Four Character Code)
pub enum FourCC {
	Iso5,
	Iso6,
	Mp41,
	Avc1,
	Av01,
	Hev1,
	Unknown([u8; 4]),
}

impl FourCC {
	pub fn to_bytes(&self) -> [u8; 4] {
		match self {
			Self::Iso5 => *b"iso5",
			Self::Iso6 => *b"iso6",
			Self::Mp41 => *b"mp41",
			Self::Avc1 => *b"avc1",
			Self::Av01 => *b"av01",
			Self::Hev1 => *b"hev1",
			Self::Unknown(bytes) => *bytes,
		}
	}
}

impl From<[u8; 4]> for FourCC {
	fn from(bytes: [u8; 4]) -> Self {
		match &bytes {
			b"iso5" => Self::Iso5,
			b"iso6" => Self::Iso6,
			b"mp41" => Self::Mp41,
			b"avc1" => Self::Avc1,
			b"av01" => Self::Av01,
			b"hev1" => Self::Hev1,
			_ => Self::Unknown(bytes),
		}
	}
}
