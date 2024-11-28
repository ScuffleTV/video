use std::fmt::{Debug, Formatter};
use std::io::{
	Read, {self},
};

use byteorder::{ReadBytesExt, WriteBytesExt};
use bytes::Bytes;
use bytesio::bytes_reader::BytesCursor;

#[derive(Clone, PartialEq)]
pub struct BoxHeader {
	pub box_type: [u8; 4],
}

impl BoxHeader {
	pub fn new(box_type: [u8; 4]) -> Self {
		Self { box_type }
	}
}

impl Debug for BoxHeader {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("BoxHeader")
			.field("box_type", &Bytes::from(self.box_type[..].to_vec()))
			.finish()
	}
}

impl BoxHeader {
	pub fn demux(reader: &mut io::Cursor<Bytes>) -> Result<(Self, Bytes), io::Error> {
		let size = reader.read_u32::<byteorder::BigEndian>()? as u64;

		let mut box_type: [u8; 4] = [0; 4];
		reader.read_exact(&mut box_type)?;

		let offset = if size == 1 { 16 } else { 8 };

		let size = if size == 1 {
			reader.read_u64::<byteorder::BigEndian>()?
		} else {
			size
		};

		let data = if size == 0 {
			// As per spec this means the box extends to the end of the file.
			reader.extract_remaining()
		} else {
			// We already read 8 bytes, so we need to subtract that from the size.
			reader.read_slice((size - offset) as usize)?
		};

		Ok((Self { box_type }, data))
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct FullBoxHeader {
	pub header: BoxHeader,
	pub version: u8,
	pub flags: u32,
}

impl FullBoxHeader {
	pub fn new(box_type: [u8; 4], version: u8, flags: u32) -> Self {
		Self {
			header: BoxHeader::new(box_type),
			version,
			flags,
		}
	}

	pub fn demux(header: BoxHeader, reader: &mut io::Cursor<Bytes>) -> io::Result<Self> {
		let version = reader.read_u8()?;
		let flags = reader.read_u24::<byteorder::BigEndian>()?;
		Ok(Self { header, version, flags })
	}

	pub fn mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		writer.write_u8(self.version)?;
		writer.write_u24::<byteorder::BigEndian>(self.flags)?;
		Ok(())
	}

	pub const fn size(&self) -> u64 {
		1 + 3
	}
}
