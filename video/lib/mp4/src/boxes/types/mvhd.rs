use std::io;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;
use fixed::types::extra::{U16, U8};
use fixed::{FixedI16, FixedI32};

use crate::boxes::header::{BoxHeader, FullBoxHeader};
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// Movie Header Box
/// ISO/IEC 14496-12:2022(E) - 8.2.2
pub struct Mvhd {
	pub header: FullBoxHeader,
	pub creation_time: u64,
	pub modification_time: u64,
	pub timescale: u32,
	pub duration: u64,
	pub rate: FixedI32<U16>,
	pub volume: FixedI16<U8>,
	pub reserved: u16,
	pub reserved2: [u32; 2],
	pub matrix: [u32; 9],
	pub pre_defined: [u32; 6],
	pub next_track_id: u32,
}

impl Mvhd {
	pub fn new(creation_time: u64, modification_time: u64, timescale: u32, duration: u64, next_track_id: u32) -> Self {
		Self {
			header: FullBoxHeader::new(Self::NAME, 0, 0),
			creation_time,
			modification_time,
			timescale,
			duration,
			rate: FixedI32::<U16>::from_num(1),
			volume: FixedI16::<U8>::from_num(1),
			reserved: 0,
			reserved2: [0; 2],
			matrix: Self::MATRIX,
			pre_defined: [0; 6],
			next_track_id,
		}
	}
}

impl Mvhd {
	pub const MATRIX: [u32; 9] = [0x00010000, 0, 0, 0, 0x00010000, 0, 0, 0, 0x40000000];
}

impl BoxType for Mvhd {
	const NAME: [u8; 4] = *b"mvhd";

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

		let rate = reader.read_i32::<BigEndian>()?;
		let volume = reader.read_i16::<BigEndian>()?;

		let reserved = reader.read_u16::<BigEndian>()?;
		let mut reserved2 = [0; 2];
		for v in reserved2.iter_mut() {
			*v = reader.read_u32::<BigEndian>()?;
		}

		let mut matrix = [0; 9];
		for v in matrix.iter_mut() {
			*v = reader.read_u32::<BigEndian>()?;
		}

		let mut pre_defined = [0; 6];
		for v in pre_defined.iter_mut() {
			*v = reader.read_u32::<BigEndian>()?;
		}

		let next_track_id = reader.read_u32::<BigEndian>()?;

		Ok(Self {
			header,
			creation_time,
			modification_time,
			timescale,
			duration,
			rate: FixedI32::from_bits(rate),
			volume: FixedI16::from_bits(volume),
			reserved,
			reserved2,
			matrix,
			pre_defined,
			next_track_id,
		})
	}

	fn primitive_size(&self) -> u64 {
		let mut size = self.header.size();

		if self.header.version == 1 {
			size += 8 + 8 + 4 + 8; // creation_time, modification_time,
			// timescale, duration
		} else {
			size += 4 + 4 + 4 + 4; // creation_time, modification_time,
			// timescale, duration
		}

		size += 4 + 2 + 2; // rate, volume, reserved
		size += 4 * 2; // reserved2
		size += 4 * 9; // matrix
		size += 4 * 6; // pre_defined
		size += 4; // next_track_id

		size
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

		writer.write_i32::<BigEndian>(self.rate.to_bits())?;
		writer.write_i16::<BigEndian>(self.volume.to_bits())?;

		writer.write_u16::<BigEndian>(self.reserved)?;

		for v in self.reserved2.iter() {
			writer.write_u32::<BigEndian>(*v)?;
		}

		for v in self.matrix.iter() {
			writer.write_u32::<BigEndian>(*v)?;
		}

		for v in self.pre_defined.iter() {
			writer.write_u32::<BigEndian>(*v)?;
		}

		writer.write_u32::<BigEndian>(self.next_track_id)?;

		Ok(())
	}

	fn validate(&self) -> io::Result<()> {
		if self.header.flags != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "mvhd flags must be 0"));
		}

		if self.header.version > 1 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "mvhd version must be 0 or 1"));
		}

		if self.header.version == 0 {
			if self.creation_time > u32::MAX as u64 {
				return Err(io::Error::new(
					io::ErrorKind::InvalidData,
					"mvhd creation_time must be less than 2^32",
				));
			}

			if self.modification_time > u32::MAX as u64 {
				return Err(io::Error::new(
					io::ErrorKind::InvalidData,
					"mvhd modification_time must be less than 2^32",
				));
			}

			if self.duration > u32::MAX as u64 {
				return Err(io::Error::new(
					io::ErrorKind::InvalidData,
					"mvhd duration must be less than 2^32",
				));
			}
		}

		if self.reserved != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "mvhd reserved must be 0"));
		}

		if self.reserved2 != [0; 2] {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "mvhd reserved2 must be 0"));
		}

		if self.pre_defined != [0; 6] {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "mvhd pre_defined must be 0"));
		}

		Ok(())
	}
}
