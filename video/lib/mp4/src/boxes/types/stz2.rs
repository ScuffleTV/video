use std::io;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;

use crate::boxes::header::{BoxHeader, FullBoxHeader};
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// Compact Sample Size Box
/// ISO/IEC 14496-12:2022(E) - 8.7.3.3
pub struct Stz2 {
	pub header: FullBoxHeader,
	pub reserved: u32,
	pub field_size: u8,
	pub samples: Vec<u16>,
}

impl BoxType for Stz2 {
	const NAME: [u8; 4] = *b"stz2";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let header = FullBoxHeader::demux(header, &mut reader)?;

		let reserved = reader.read_u24::<BigEndian>()?;

		let field_size = reader.read_u8()?;
		let sample_count = reader.read_u32::<BigEndian>()?;

		let mut samples = Vec::with_capacity(sample_count as usize);

		let mut sample_idx = 0;
		while sample_idx < sample_count {
			let sample = match field_size {
				4 => {
					let byte = reader.read_u8()?;
					samples.push((byte >> 4) as u16);
					sample_idx += 1;
					if sample_idx >= sample_count {
						break;
					}

					(byte & 0x0F) as u16
				}
				8 => reader.read_u8()? as u16,
				16 => reader.read_u16::<BigEndian>()?,
				_ => {
					return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid field size"));
				}
			};

			sample_idx += 1;
			samples.push(sample);
		}

		Ok(Self {
			header,
			reserved,
			field_size,
			samples,
		})
	}

	fn primitive_size(&self) -> u64 {
		let size = self.header.size();
		let size = size + 8; // reserved + field_size + sample_count

		size + (self.samples.len() as u64 * self.field_size as u64)
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.header.mux(writer)?;

		writer.write_u24::<BigEndian>(self.reserved)?;
		writer.write_u8(self.field_size)?;
		writer.write_u32::<BigEndian>(self.samples.len() as u32)?;

		let mut sample_idx = 0;
		while sample_idx < self.samples.len() {
			let sample = self.samples[sample_idx];
			match self.field_size {
				4 => {
					let byte = (sample << 4) as u8;
					sample_idx += 1;
					if sample_idx >= self.samples.len() {
						writer.write_u8(byte)?;
						break;
					}

					let sample = self.samples[sample_idx];
					let byte = byte | (sample & 0x0F) as u8;
					writer.write_u8(byte)?;
				}
				8 => writer.write_u8(sample as u8)?,
				16 => writer.write_u16::<BigEndian>(sample)?,
				_ => {
					return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid field size"));
				}
			};

			sample_idx += 1;
		}

		Ok(())
	}

	fn validate(&self) -> io::Result<()> {
		if self.header.version != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "stz2 version must be 0"));
		}

		if self.reserved != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "stz2 reserved must be 0"));
		}

		if self.field_size != 4 && self.field_size != 8 && self.field_size != 16 {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"stz2 field_size must be 4, 8 or 16",
			));
		}

		if self.field_size != 16 {
			for sample in &self.samples {
				if self.field_size == 4 {
					if *sample > 0x0F {
						return Err(io::Error::new(io::ErrorKind::InvalidData, "stz2 sample value must be 4 bits"));
					}
				} else if self.field_size == 8 && *sample > 0xFF {
					return Err(io::Error::new(io::ErrorKind::InvalidData, "stz2 sample value must be 8 bits"));
				}
			}
		}

		Ok(())
	}
}
