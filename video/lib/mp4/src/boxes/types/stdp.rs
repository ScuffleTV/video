use std::io;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::{Buf, Bytes};

use crate::boxes::header::{BoxHeader, FullBoxHeader};
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// Sample Degradation Priority Box
/// ISO/IEC 14496-12:2022(E) - 8.7.6
pub struct Stdp {
	pub header: FullBoxHeader,
	pub samples: Vec<u16>,
}

impl BoxType for Stdp {
	const NAME: [u8; 4] = *b"stdp";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let header = FullBoxHeader::demux(header, &mut reader)?;

		let mut samples = Vec::new();

		while reader.remaining() > 1 {
			let sample = reader.read_u16::<BigEndian>()?;
			samples.push(sample);
		}

		Ok(Self { header, samples })
	}

	fn primitive_size(&self) -> u64 {
		let size = self.header.size();
		// samples
		size + (self.samples.len() as u64) * 2
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.header.mux(writer)?;

		for sample in &self.samples {
			writer.write_u16::<BigEndian>(*sample)?;
		}

		Ok(())
	}

	fn validate(&self) -> io::Result<()> {
		if self.header.version != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "stdp version must be 0"));
		}

		if self.header.flags != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "stdp flags must be 0"));
		}

		Ok(())
	}
}
