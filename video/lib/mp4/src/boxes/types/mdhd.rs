use std::io;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;

use crate::boxes::header::{BoxHeader, FullBoxHeader};
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// Media Header Box
/// ISO/IEC 14496-12:2022(E) - 8.4.2
pub struct Mdhd {
	pub header: FullBoxHeader,
	pub creation_time: u64,
	pub modification_time: u64,
	pub timescale: u32,
	pub duration: u64,
	pub language: u16,
	pub pre_defined: u16,
}

impl Mdhd {
	pub fn new(creation_time: u64, modification_time: u64, timescale: u32, duration: u64) -> Self {
		let version = if creation_time > u32::MAX as u64 || modification_time > u32::MAX as u64 || duration > u32::MAX as u64
		{
			1
		} else {
			0
		};

		Self {
			header: FullBoxHeader::new(Self::NAME, version, 0),
			creation_time,
			modification_time,
			timescale,
			duration,
			language: 0x55c4, // und
			pre_defined: 0,
		}
	}
}

impl BoxType for Mdhd {
	const NAME: [u8; 4] = *b"mdhd";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let header = FullBoxHeader::demux(header, &mut reader)?;

		let (creation_time, modification_time, timescale, duration) = if header.version == 1 {
			(
				reader.read_u64::<BigEndian>()?, // creation_time
				reader.read_u64::<BigEndian>()?, // modification_time
				reader.read_u32::<BigEndian>()?, // timescale
				reader.read_u64::<BigEndian>()?, // duration
			)
		} else {
			(
				reader.read_u32::<BigEndian>()? as u64, // creation_time
				reader.read_u32::<BigEndian>()? as u64, // modification_time
				reader.read_u32::<BigEndian>()?,        // timescale
				reader.read_u32::<BigEndian>()? as u64, // duration
			)
		};

		let language = reader.read_u16::<BigEndian>()?;
		let pre_defined = reader.read_u16::<BigEndian>()?;

		Ok(Self {
			header,
			creation_time,
			modification_time,
			timescale,
			duration,
			language,
			pre_defined,
		})
	}

	fn primitive_size(&self) -> u64 {
		self.header.size()
        + if self.header.version == 1 {
            8 + 8 + 4 + 8 // creation_time + modification_time + timescale + duration
        } else {
            4 + 4 + 4 + 4 // creation_time + modification_time + timescale + duration
        }
        + 2 // language
        + 2 // pre_defined
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.header.mux(writer)?;

		if self.header.version == 1 {
			writer.write_u64::<BigEndian>(self.creation_time)?;
			writer.write_u64::<BigEndian>(self.modification_time)?;
			writer.write_u32::<BigEndian>(self.timescale)?;
			writer.write_u64::<BigEndian>(self.duration)?;
		} else {
			writer.write_u32::<BigEndian>(self.creation_time as u32)?;
			writer.write_u32::<BigEndian>(self.modification_time as u32)?;
			writer.write_u32::<BigEndian>(self.timescale)?;
			writer.write_u32::<BigEndian>(self.duration as u32)?;
		}

		writer.write_u16::<BigEndian>(self.language)?;
		writer.write_u16::<BigEndian>(self.pre_defined)?;

		Ok(())
	}

	fn validate(&self) -> io::Result<()> {
		if self.header.flags != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "mdhd box flags must be 0"));
		}

		if self.header.version > 1 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "mdhd box version must be 0 or 1"));
		}

		if self.header.version == 0 {
			if self.creation_time > u32::MAX as u64 {
				return Err(io::Error::new(
					io::ErrorKind::InvalidData,
					"mdhd box creation_time must be less than 2^32",
				));
			}

			if self.modification_time > u32::MAX as u64 {
				return Err(io::Error::new(
					io::ErrorKind::InvalidData,
					"mdhd box modification_time must be less than 2^32",
				));
			}

			if self.duration > u32::MAX as u64 {
				return Err(io::Error::new(
					io::ErrorKind::InvalidData,
					"mdhd box duration must be less than 2^32",
				));
			}
		}

		if self.pre_defined != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "mdhd box pre_defined must be 0"));
		}

		Ok(())
	}
}
