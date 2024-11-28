use std::io;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;
use fixed::types::extra::{U16, U8};
use fixed::{FixedI16, FixedI32};

use crate::boxes::header::{BoxHeader, FullBoxHeader};
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// Track Header Box
/// ISO/IEC 14496-12:2022(E) - 8.3.2
pub struct Tkhd {
	pub header: FullBoxHeader,
	pub creation_time: u64,
	pub modification_time: u64,
	pub track_id: u32,
	pub reserved: u32,
	pub duration: u64,
	pub reserved2: [u32; 2],
	pub layer: u16,
	pub alternate_group: u16,
	pub volume: FixedI16<U8>,
	pub reserved3: u16,
	pub matrix: [u32; 9],
	pub width: FixedI32<U16>,
	pub height: FixedI32<U16>,
}

impl Tkhd {
	pub fn new(
		creation_time: u64,
		modification_time: u64,
		track_id: u32,
		duration: u64,
		width_height: Option<(u32, u32)>,
	) -> Self {
		let version = if creation_time > u32::MAX as u64 || modification_time > u32::MAX as u64 || duration > u32::MAX as u64
		{
			1
		} else {
			0
		};

		let (width, height) = width_height.unwrap_or((0, 0));
		let volume = if width_height.is_some() {
			FixedI16::<U8>::from_num(0)
		} else {
			FixedI16::<U8>::from_num(1)
		};

		Self {
			header: FullBoxHeader::new(Self::NAME, version, Self::TRACK_ENABLED_FLAG | Self::TRACK_IN_MOVIE_FLAG),
			creation_time,
			modification_time,
			track_id,
			reserved: 0,
			duration,
			reserved2: [0; 2],
			layer: 0,
			alternate_group: 0,
			volume,
			reserved3: 0,
			matrix: Self::MATRIX,
			width: FixedI32::<U16>::from_num(width),
			height: FixedI32::<U16>::from_num(height),
		}
	}
}

impl Tkhd {
	pub const MATRIX: [u32; 9] = [0x00010000, 0, 0, 0, 0x00010000, 0, 0, 0, 0x40000000];
	pub const TRACK_ENABLED_FLAG: u32 = 0x000001;
	pub const TRACK_IN_MOVIE_FLAG: u32 = 0x000002;
	pub const TRACK_IN_PREVIEW_FLAG: u32 = 0x000004;
	pub const TRACK_SIZE_IS_ASPECT_RATIO_FLAG: u32 = 0x000008;
}

impl BoxType for Tkhd {
	const NAME: [u8; 4] = *b"tkhd";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let header = FullBoxHeader::demux(header, &mut reader)?;

		let (creation_time, modification_time, track_id, reserved, duration) = if header.version == 1 {
			(
				reader.read_u64::<BigEndian>()?, // creation_time
				reader.read_u64::<BigEndian>()?, // modification_time
				reader.read_u32::<BigEndian>()?, // track_id
				reader.read_u32::<BigEndian>()?, // reserved
				reader.read_u64::<BigEndian>()?, // duration
			)
		} else {
			(
				reader.read_u32::<BigEndian>()? as u64, // creation_time
				reader.read_u32::<BigEndian>()? as u64, // modification_time
				reader.read_u32::<BigEndian>()?,        // track_id
				reader.read_u32::<BigEndian>()?,        // reserved
				reader.read_u32::<BigEndian>()? as u64, // duration
			)
		};

		let mut reserved2 = [0; 2];
		for v in reserved2.iter_mut() {
			*v = reader.read_u32::<BigEndian>()?;
		}

		let layer = reader.read_u16::<BigEndian>()?;
		let alternate_group = reader.read_u16::<BigEndian>()?;
		let volume = reader.read_i16::<BigEndian>()?;

		let reserved3 = reader.read_u16::<BigEndian>()?;

		let mut matrix = [0; 9];
		for v in matrix.iter_mut() {
			*v = reader.read_u32::<BigEndian>()?;
		}

		let width = reader.read_i32::<BigEndian>()?;
		let height = reader.read_i32::<BigEndian>()?;

		Ok(Self {
			header,
			creation_time,
			modification_time,
			track_id,
			reserved,
			duration,
			reserved2,
			layer,
			alternate_group,
			volume: FixedI16::<U8>::from_bits(volume),
			reserved3,
			matrix,
			width: FixedI32::<U16>::from_bits(width),
			height: FixedI32::<U16>::from_bits(height),
		})
	}

	fn primitive_size(&self) -> u64 {
		let mut size = self.header.size();
		size += if self.header.version == 1 {
			8 + 8 + 4 + 4 + 8 // creation_time, modification_time, track_id,
			// reserved, duration
		} else {
			4 + 4 + 4 + 4 + 4 // creation_time, modification_time, track_id,
			// reserved, duration
		};

		size += 4 * 2; // reserved2
		size += 2; // layer
		size += 2; // alternate_group
		size += 2; // volume
		size += 2; // reserved
		size += 4 * 9; // matrix
		size += 4; // width
		size += 4; // height

		size
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.header.mux(writer)?;

		if self.header.version == 1 {
			writer.write_u64::<BigEndian>(self.creation_time)?;
			writer.write_u64::<BigEndian>(self.modification_time)?;
			writer.write_u32::<BigEndian>(self.track_id)?;
			writer.write_u32::<BigEndian>(self.reserved)?;
			writer.write_u64::<BigEndian>(self.duration)?;
		} else {
			writer.write_u32::<BigEndian>(self.creation_time as u32)?;
			writer.write_u32::<BigEndian>(self.modification_time as u32)?;
			writer.write_u32::<BigEndian>(self.track_id)?;
			writer.write_u32::<BigEndian>(self.reserved)?;
			writer.write_u32::<BigEndian>(self.duration as u32)?;
		}

		for v in self.reserved2.iter() {
			writer.write_u32::<BigEndian>(*v)?;
		}

		writer.write_u16::<BigEndian>(self.layer)?;
		writer.write_u16::<BigEndian>(self.alternate_group)?;
		writer.write_i16::<BigEndian>(self.volume.to_bits())?;
		writer.write_u16::<BigEndian>(self.reserved3)?;

		for v in self.matrix.iter() {
			writer.write_u32::<BigEndian>(*v)?;
		}

		writer.write_i32::<BigEndian>(self.width.to_bits())?;
		writer.write_i32::<BigEndian>(self.height.to_bits())?;

		Ok(())
	}

	fn validate(&self) -> io::Result<()> {
		if self.header.version > 1 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "tkhd version must be 0 or 1"));
		}

		if self.track_id == 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "tkhd track_id must not be 0"));
		}

		if self.reserved != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "tkhd reserved must be 0"));
		}

		if self.reserved2 != [0; 2] {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "tkhd reserved2 must be 0"));
		}

		if self.reserved3 != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "tkhd reserved3 must be 0"));
		}

		if self.header.version == 0 {
			if self.creation_time > u32::MAX as u64 {
				return Err(io::Error::new(
					io::ErrorKind::InvalidData,
					"tkhd creation_time must be less than 2^32",
				));
			}

			if self.modification_time > u32::MAX as u64 {
				return Err(io::Error::new(
					io::ErrorKind::InvalidData,
					"tkhd modification_time must be less than 2^32",
				));
			}

			if self.duration > u32::MAX as u64 {
				return Err(io::Error::new(
					io::ErrorKind::InvalidData,
					"tkhd duration must be less than 2^32",
				));
			}
		}

		Ok(())
	}
}
