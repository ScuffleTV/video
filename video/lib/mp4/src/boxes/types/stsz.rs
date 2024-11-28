use std::io;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;

use crate::boxes::header::{BoxHeader, FullBoxHeader};
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// Sample Size Box
/// ISO/IEC 14496-12:2022(E) - 8.7.3.2
pub struct Stsz {
	pub header: FullBoxHeader,
	pub sample_size: u32,
	pub samples: Vec<u32>,
}

impl Stsz {
	pub fn new(sample_size: u32, samples: Vec<u32>) -> Self {
		Self {
			header: FullBoxHeader::new(Self::NAME, 0, 0),
			sample_size,
			samples,
		}
	}
}

impl BoxType for Stsz {
	const NAME: [u8; 4] = *b"stsz";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let header = FullBoxHeader::demux(header, &mut reader)?;

		let sample_size = reader.read_u32::<BigEndian>()?;
		let sample_count = reader.read_u32::<BigEndian>()?;

		let mut samples = Vec::with_capacity(sample_count as usize);
		if sample_size == 0 {
			for _ in 0..sample_count {
				let size = reader.read_u32::<BigEndian>()?;
				samples.push(size);
			}
		}

		Ok(Self {
			header,
			sample_size,
			samples,
		})
	}

	fn primitive_size(&self) -> u64 {
		let size = self.header.size();
		let size = size + 8; // sample_size + sample_count

		if self.sample_size == 0 {
			size + (self.samples.len() as u64 * 4) // samples
		} else {
			size
		}
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.header.mux(writer)?;

		writer.write_u32::<BigEndian>(self.sample_size)?;
		writer.write_u32::<BigEndian>(self.samples.len() as u32)?;

		if self.sample_size == 0 {
			for size in &self.samples {
				writer.write_u32::<BigEndian>(*size)?;
			}
		}

		Ok(())
	}

	fn validate(&self) -> io::Result<()> {
		if self.sample_size != 0 && !self.samples.is_empty() {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"stsz: sample_size is not 0 but samples are present",
			));
		}

		if self.header.version != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "stsz box version must be 0"));
		}

		if self.header.flags != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "stsz box flags must be 0"));
		}

		Ok(())
	}
}
