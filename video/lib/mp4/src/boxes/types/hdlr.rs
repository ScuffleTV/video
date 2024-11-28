use std::io::{
	Read, {self},
};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;

use crate::boxes::header::{BoxHeader, FullBoxHeader};
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// Handler Reference Box
/// ISO/IEC 14496-12:2022(E) - 8.4.3
pub struct Hdlr {
	pub header: FullBoxHeader,
	pub pre_defined: u32,
	pub handler_type: HandlerType,
	pub reserved: [u32; 3],
	pub name: String,
}

impl Hdlr {
	pub fn new(handler_type: HandlerType, name: String) -> Self {
		Self {
			header: FullBoxHeader::new(Self::NAME, 0, 0),
			pre_defined: 0,
			handler_type,
			reserved: [0; 3],
			name,
		}
	}
}

#[derive(Debug, Clone, PartialEq)]
/// The handler type indicates the media type of the media in the track.
/// The handler type is a 32-bit value composed of a 4-character code.
pub enum HandlerType {
	Vide,
	Soun,
	Hint,
	Meta,
	Unknown([u8; 4]),
}

impl HandlerType {
	pub fn to_bytes(&self) -> [u8; 4] {
		match self {
			Self::Vide => *b"vide",
			Self::Soun => *b"soun",
			Self::Hint => *b"hint",
			Self::Meta => *b"meta",
			Self::Unknown(b) => *b,
		}
	}
}

impl From<[u8; 4]> for HandlerType {
	fn from(v: [u8; 4]) -> Self {
		match &v {
			b"vide" => Self::Vide,
			b"soun" => Self::Soun,
			b"hint" => Self::Hint,
			b"meta" => Self::Meta,
			_ => Self::Unknown(v),
		}
	}
}

impl BoxType for Hdlr {
	const NAME: [u8; 4] = *b"hdlr";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let header = FullBoxHeader::demux(header, &mut reader)?;

		let pre_defined = reader.read_u32::<BigEndian>()?;

		let mut handler_type = [0; 4];
		reader.read_exact(&mut handler_type)?;

		let mut reserved = [0; 3];
		for v in reserved.iter_mut() {
			*v = reader.read_u32::<BigEndian>()?;
		}

		let mut name = String::new();
		loop {
			let c = reader.read_u8()?;
			if c == 0 {
				break;
			}

			name.push(c as char);
		}

		Ok(Self {
			header,
			pre_defined,
			handler_type: handler_type.into(),
			reserved,
			name,
		})
	}

	fn primitive_size(&self) -> u64 {
		self.header.size()
        + 4 // pre_defined
        + 4 // handler_type
        + 3 * 4 // reserved
        + self.name.len() as u64 + 1 // name + null terminator
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.header.mux(writer)?;

		writer.write_u32::<BigEndian>(self.pre_defined)?;

		writer.write_all(&self.handler_type.to_bytes())?;

		for v in self.reserved.iter() {
			writer.write_u32::<BigEndian>(*v)?;
		}

		writer.write_all(self.name.as_bytes())?;
		writer.write_u8(0)?;

		Ok(())
	}

	fn validate(&self) -> io::Result<()> {
		if self.header.version != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "hdlr version must be 0"));
		}

		if self.header.flags != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "hdlr flags must be 0"));
		}

		if self.reserved != [0; 3] {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "hdlr reserved must be 0"));
		}

		if self.pre_defined != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "hdlr pre_defined must be 0"));
		}

		Ok(())
	}
}
