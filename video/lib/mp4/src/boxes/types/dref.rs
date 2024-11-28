use std::io;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;

use super::url::Url;
use crate::boxes::header::{BoxHeader, FullBoxHeader};
use crate::boxes::traits::BoxType;
use crate::boxes::DynBox;

#[derive(Debug, Clone, PartialEq)]
/// Data Reference Box
/// IEO/IEC 14496-12:2022(E) - 8.7.2
pub struct Dref {
	pub header: FullBoxHeader,
	pub entries: Vec<DynBox>,
}

impl Default for Dref {
	fn default() -> Self {
		Self::new()
	}
}

impl Dref {
	pub fn new() -> Self {
		Self {
			header: FullBoxHeader::new(Self::NAME, 0, 0),
			entries: vec![Url::new().into()],
		}
	}
}

impl BoxType for Dref {
	const NAME: [u8; 4] = *b"dref";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let header = FullBoxHeader::demux(header, &mut reader)?;

		let entry_count = reader.read_u32::<BigEndian>()?;

		let mut entries = Vec::new();

		for _ in 0..entry_count {
			let dyn_box = DynBox::demux(&mut reader)?;
			entries.push(dyn_box);
		}

		Ok(Self { header, entries })
	}

	fn primitive_size(&self) -> u64 {
		self.header.size()
        + 4 // entry_count
        + self.entries.iter().map(|b| b.size()).sum::<u64>()
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.header.mux(writer)?;

		writer.write_u32::<BigEndian>(self.entries.len() as u32)?;

		for b in &self.entries {
			b.mux(writer)?;
		}

		Ok(())
	}

	fn validate(&self) -> io::Result<()> {
		if self.header.version != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "dref version must be 0"));
		}

		if self.header.flags != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "dref flags must be 0"));
		}

		Ok(())
	}
}
