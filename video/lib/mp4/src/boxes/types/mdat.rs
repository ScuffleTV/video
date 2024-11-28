use std::io;

use bytes::Bytes;

use crate::boxes::header::BoxHeader;
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// Media Data Box
/// ISO/IEC 14496-12:2022(E) - 8.2.2
pub struct Mdat {
	pub header: BoxHeader,
	pub data: Vec<Bytes>,
}

impl Mdat {
	pub fn new(data: Vec<Bytes>) -> Self {
		Self {
			header: BoxHeader::new(Self::NAME),
			data,
		}
	}
}

impl BoxType for Mdat {
	const NAME: [u8; 4] = *b"mdat";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		Ok(Self {
			header,
			data: vec![data],
		})
	}

	fn primitive_size(&self) -> u64 {
		self.data.iter().map(|data| data.len() as u64).sum()
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		for data in &self.data {
			writer.write_all(data)?;
		}

		Ok(())
	}
}
