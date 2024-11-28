use std::io;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;

use super::trun::TrunSampleFlag;
use crate::boxes::header::{BoxHeader, FullBoxHeader};
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// Track Fragment Header Box
/// ISO/IEC 14496-12:2022(E) - 8.8.7
pub struct Tfhd {
	pub header: FullBoxHeader,
	pub track_id: u32,
	pub base_data_offset: Option<u64>,
	pub sample_description_index: Option<u32>,
	pub default_sample_duration: Option<u32>,
	pub default_sample_size: Option<u32>,
	pub default_sample_flags: Option<TrunSampleFlag>,
}

impl Tfhd {
	pub const BASE_DATA_OFFSET_FLAG: u32 = 0x000001;
	pub const DEFAULT_BASE_IS_MOOF_FLAG: u32 = 0x020000;
	pub const DEFAULT_SAMPLE_DURATION_FLAG: u32 = 0x000008;
	pub const DEFAULT_SAMPLE_FLAGS_FLAG: u32 = 0x000020;
	pub const DEFAULT_SAMPLE_SIZE_FLAG: u32 = 0x000010;
	pub const DURATION_IS_EMPTY_FLAG: u32 = 0x010000;
	pub const SAMPLE_DESCRIPTION_INDEX_FLAG: u32 = 0x000002;

	pub fn new(
		track_id: u32,
		base_data_offset: Option<u64>,
		sample_description_index: Option<u32>,
		default_sample_duration: Option<u32>,
		default_sample_size: Option<u32>,
		default_sample_flags: Option<TrunSampleFlag>,
	) -> Self {
		let flags = if base_data_offset.is_some() {
			Self::BASE_DATA_OFFSET_FLAG
		} else {
			0
		} | if sample_description_index.is_some() {
			Self::SAMPLE_DESCRIPTION_INDEX_FLAG
		} else {
			0
		} | if default_sample_duration.is_some() {
			Self::DEFAULT_SAMPLE_DURATION_FLAG
		} else {
			0
		} | if default_sample_size.is_some() {
			Self::DEFAULT_SAMPLE_SIZE_FLAG
		} else {
			0
		} | if default_sample_flags.is_some() {
			Self::DEFAULT_SAMPLE_FLAGS_FLAG
		} else {
			0
		} | Self::DEFAULT_BASE_IS_MOOF_FLAG;

		Self {
			header: FullBoxHeader::new(Self::NAME, 0, flags),
			track_id,
			base_data_offset,
			sample_description_index,
			default_sample_duration,
			default_sample_size,
			default_sample_flags,
		}
	}
}

impl BoxType for Tfhd {
	const NAME: [u8; 4] = *b"tfhd";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let header = FullBoxHeader::demux(header, &mut reader)?;

		let track_id = reader.read_u32::<BigEndian>()?;

		let base_data_offset = if header.flags & Self::BASE_DATA_OFFSET_FLAG != 0 {
			Some(reader.read_u64::<BigEndian>()?)
		} else {
			None
		};

		let sample_description_index = if header.flags & Self::SAMPLE_DESCRIPTION_INDEX_FLAG != 0 {
			Some(reader.read_u32::<BigEndian>()?)
		} else {
			None
		};

		let default_sample_duration = if header.flags & Self::DEFAULT_SAMPLE_DURATION_FLAG != 0 {
			Some(reader.read_u32::<BigEndian>()?)
		} else {
			None
		};

		let default_sample_size = if header.flags & Self::DEFAULT_SAMPLE_SIZE_FLAG != 0 {
			Some(reader.read_u32::<BigEndian>()?)
		} else {
			None
		};

		let default_sample_flags = if header.flags & Self::DEFAULT_SAMPLE_FLAGS_FLAG != 0 {
			Some(reader.read_u32::<BigEndian>()?.into())
		} else {
			None
		};

		Ok(Self {
			header,
			track_id,
			base_data_offset,
			sample_description_index,
			default_sample_duration,
			default_sample_size,
			default_sample_flags,
		})
	}

	fn primitive_size(&self) -> u64 {
		self.header.size()
			+ 4 + self.base_data_offset.map_or(0, |_| 8)
			+ self.sample_description_index.map_or(0, |_| 4)
			+ self.default_sample_duration.map_or(0, |_| 4)
			+ self.default_sample_size.map_or(0, |_| 4)
			+ self.default_sample_flags.map_or(0, |_| 4)
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.header.mux(writer)?;

		writer.write_u32::<BigEndian>(self.track_id)?;

		if let Some(base_data_offset) = self.base_data_offset {
			writer.write_u64::<BigEndian>(base_data_offset)?;
		}

		if let Some(sample_description_index) = self.sample_description_index {
			writer.write_u32::<BigEndian>(sample_description_index)?;
		}

		if let Some(default_sample_duration) = self.default_sample_duration {
			writer.write_u32::<BigEndian>(default_sample_duration)?;
		}

		if let Some(default_sample_size) = self.default_sample_size {
			writer.write_u32::<BigEndian>(default_sample_size)?;
		}

		if let Some(default_sample_flags) = self.default_sample_flags {
			writer.write_u32::<BigEndian>(default_sample_flags.into())?;
		}

		Ok(())
	}

	fn validate(&self) -> io::Result<()> {
		if self.header.version != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "tfhd version must be 0"));
		}

		if self.header.flags & Self::BASE_DATA_OFFSET_FLAG != 0 && self.base_data_offset.is_none() {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"tfhd base_data_offset flag is set but base_data_offset is not present",
			));
		} else if self.header.flags & Self::BASE_DATA_OFFSET_FLAG == 0 && self.base_data_offset.is_some() {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"tfhd base_data_offset flag is not set but base_data_offset is present",
			));
		}

		if self.header.flags & Self::SAMPLE_DESCRIPTION_INDEX_FLAG != 0 && self.sample_description_index.is_none() {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"tfhd sample_description_index flag is set but sample_description_index is not present",
			));
		} else if self.header.flags & Self::SAMPLE_DESCRIPTION_INDEX_FLAG == 0 && self.sample_description_index.is_some() {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"tfhd sample_description_index flag is not set but sample_description_index is present",
			));
		}

		if self.header.flags & Self::DEFAULT_SAMPLE_DURATION_FLAG != 0 && self.default_sample_duration.is_none() {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"tfhd default_sample_duration flag is set but default_sample_duration is not present",
			));
		} else if self.header.flags & Self::DEFAULT_SAMPLE_DURATION_FLAG == 0 && self.default_sample_duration.is_some() {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"tfhd default_sample_duration flag is not set but default_sample_duration is present",
			));
		}

		if self.header.flags & Self::DEFAULT_SAMPLE_SIZE_FLAG != 0 && self.default_sample_size.is_none() {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"tfhd default_sample_size flag is set but default_sample_size is not present",
			));
		} else if self.header.flags & Self::DEFAULT_SAMPLE_SIZE_FLAG == 0 && self.default_sample_size.is_some() {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"tfhd default_sample_size flag is not set but default_sample_size is present",
			));
		}

		if self.header.flags & Self::DEFAULT_SAMPLE_FLAGS_FLAG != 0 && self.default_sample_flags.is_none() {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"tfhd default_sample_flags flag is set but default_sample_flags is not present",
			));
		} else if self.header.flags & Self::DEFAULT_SAMPLE_FLAGS_FLAG == 0 && self.default_sample_flags.is_some() {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"tfhd default_sample_flags flag is not set but default_sample_flags is present",
			));
		}

		if let Some(default_sample_flags) = self.default_sample_flags {
			default_sample_flags.validate()?;
		}

		Ok(())
	}
}
