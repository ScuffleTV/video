use std::io;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;

use crate::boxes::header::{BoxHeader, FullBoxHeader};
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// Padding bits box.
/// ISO/IEC 14496-12:2022(E) - 8.7.6
pub struct Padb {
	pub header: FullBoxHeader,
	pub samples: Vec<u8>,
}

impl BoxType for Padb {
	const NAME: [u8; 4] = *b"padb";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let header = FullBoxHeader::demux(header, &mut reader)?;

		let sample_count = (reader.read_u32::<BigEndian>()? + 1) / 2;
		let mut samples = Vec::with_capacity(sample_count as usize);

		for _ in 0..sample_count {
			let byte = reader.read_u8()?;
			samples.push(byte);
		}

		Ok(Self { header, samples })
	}

	fn primitive_size(&self) -> u64 {
		let mut size = self.header.size();
		size += 4; // sample_count
		size + (self.samples.len() as u64) // samples
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.header.mux(writer)?;

		writer.write_u32::<BigEndian>((self.samples.len() as u32) * 2 - 1)?;

		for byte in &self.samples {
			writer.write_u8(*byte)?;
		}

		Ok(())
	}

	fn validate(&self) -> io::Result<()> {
		if self.header.version != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "padb box version must be 0"));
		}

		if self.header.flags != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "padb box flags must be 0"));
		}

		Ok(())
	}
}
