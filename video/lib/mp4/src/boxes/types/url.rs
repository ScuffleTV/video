use std::io;

use byteorder::{ReadBytesExt, WriteBytesExt};
use bytes::Bytes;

use crate::boxes::header::{BoxHeader, FullBoxHeader};
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// Url Box
/// ISO/IEC 14496-12:2022(E) - 8.7.2.2
pub struct Url {
	pub header: FullBoxHeader,
	pub location: Option<String>,
}

impl Default for Url {
	fn default() -> Self {
		Self::new()
	}
}

impl Url {
	pub fn new() -> Self {
		Self {
			header: FullBoxHeader::new(Self::NAME, 0, 1),
			location: None,
		}
	}
}

impl BoxType for Url {
	const NAME: [u8; 4] = *b"url ";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let header = FullBoxHeader::demux(header, &mut reader)?;

		let location = if header.flags == 0 {
			let mut location = String::new();
			loop {
				let byte = reader.read_u8()?;
				if byte == 0 {
					break;
				}
				location.push(byte as char);
			}

			Some(location)
		} else {
			None
		};

		Ok(Self { header, location })
	}

	fn primitive_size(&self) -> u64 {
		self.header.size()
			+ if let Some(location) = &self.location {
				location.len() as u64 + 1 // null terminator
			} else {
				0
			}
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.header.mux(writer)?;

		if let Some(location) = &self.location {
			writer.write_all(location.as_bytes())?;
			writer.write_u8(0)?;
		}

		Ok(())
	}

	fn validate(&self) -> io::Result<()> {
		if self.header.version != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "url version must be 0"));
		}

		if self.header.flags != 0 && self.location.is_some() {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"url location must be empty if flags is 1",
			));
		} else if self.header.flags == 0 && self.location.is_none() {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"url location must be present if flags is 0",
			));
		}

		Ok(())
	}
}
