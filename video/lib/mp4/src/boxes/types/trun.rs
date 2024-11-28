use std::io;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;

use crate::boxes::header::{BoxHeader, FullBoxHeader};
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// Track Fragment Run Box
/// ISO/IEC 14496-12:2022(E) - 8.8.8
pub struct Trun {
	pub header: FullBoxHeader,
	pub data_offset: Option<i32>,
	pub first_sample_flags: Option<TrunSampleFlag>,
	pub samples: Vec<TrunSample>,
}

impl Trun {
	pub fn new(samples: Vec<TrunSample>, first_sample_flags: Option<TrunSampleFlag>) -> Self {
		let flags = if samples.is_empty() {
			Self::FLAG_DATA_OFFSET
		} else {
			let mut flags = Self::FLAG_DATA_OFFSET;
			if samples[0].duration.is_some() {
				flags |= Self::FLAG_SAMPLE_DURATION;
			}
			if samples[0].size.is_some() {
				flags |= Self::FLAG_SAMPLE_SIZE;
			}
			if samples[0].flags.is_some() {
				flags |= Self::FLAG_SAMPLE_FLAGS;
			}
			if samples[0].composition_time_offset.is_some() {
				flags |= Self::FLAG_SAMPLE_COMPOSITION_TIME_OFFSET;
			}
			flags
		};

		let version = if samples
			.iter()
			.any(|s| s.composition_time_offset.is_some() && s.composition_time_offset.unwrap() < 0)
		{
			1
		} else {
			0
		};

		Self {
			header: FullBoxHeader::new(Self::NAME, version, flags),
			data_offset: Some(0),
			first_sample_flags,
			samples,
		}
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrunSample {
	pub duration: Option<u32>,
	pub size: Option<u32>,
	pub flags: Option<TrunSampleFlag>,
	pub composition_time_offset: Option<i64>, // we use i64 because it is either a u32 or a i32
}

#[derive(Debug, Clone, PartialEq, Copy, Default)]
pub struct TrunSampleFlag {
	pub reserved: u8,                     // 4 bits
	pub is_leading: u8,                   // 2 bits
	pub sample_depends_on: u8,            // 2 bits
	pub sample_is_depended_on: u8,        // 2 bits
	pub sample_has_redundancy: u8,        // 2 bits
	pub sample_padding_value: u8,         // 3 bits
	pub sample_is_non_sync_sample: bool,  // 1 bit
	pub sample_degradation_priority: u16, // 16 bits
}

impl TrunSampleFlag {
	pub fn validate(&self) -> io::Result<()> {
		if self.reserved != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "reserved bits must be 0"));
		}

		if self.is_leading > 2 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "is_leading must be 0, 1 or 2"));
		}

		if self.sample_depends_on > 2 {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"sample_depends_on must be 0, 1 or 2",
			));
		}

		if self.sample_is_depended_on > 2 {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"sample_is_depended_on must be 0, 1 or 2",
			));
		}

		if self.sample_has_redundancy > 2 {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"sample_has_redundancy must be 0, 1 or 2",
			));
		}

		if self.sample_padding_value > 7 {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"sample_padding_value must be 0, 1, 2, 3, 4, 5, 6 or 7",
			));
		}

		Ok(())
	}
}

impl From<u32> for TrunSampleFlag {
	fn from(value: u32) -> Self {
		let reserved = (value >> 28) as u8;
		let is_leading = ((value >> 26) & 0b11) as u8;
		let sample_depends_on = ((value >> 24) & 0b11) as u8;
		let sample_is_depended_on = ((value >> 22) & 0b11) as u8;
		let sample_has_redundancy = ((value >> 20) & 0b11) as u8;
		let sample_padding_value = ((value >> 17) & 0b111) as u8;
		let sample_is_non_sync_sample = ((value >> 16) & 0b1) != 0;
		let sample_degradation_priority = (value & 0xFFFF) as u16;

		Self {
			reserved,
			is_leading,
			sample_depends_on,
			sample_is_depended_on,
			sample_has_redundancy,
			sample_padding_value,
			sample_is_non_sync_sample,
			sample_degradation_priority,
		}
	}
}

impl From<TrunSampleFlag> for u32 {
	fn from(value: TrunSampleFlag) -> Self {
		let mut result = 0;

		result |= (value.reserved as u32) << 28;
		result |= (value.is_leading as u32) << 26;
		result |= (value.sample_depends_on as u32) << 24;
		result |= (value.sample_is_depended_on as u32) << 22;
		result |= (value.sample_has_redundancy as u32) << 20;
		result |= (value.sample_padding_value as u32) << 17;
		result |= (value.sample_is_non_sync_sample as u32) << 16;
		result |= value.sample_degradation_priority as u32;

		result
	}
}

impl Trun {
	pub const FLAG_DATA_OFFSET: u32 = 0x000001;
	pub const FLAG_FIRST_SAMPLE_FLAGS: u32 = 0x000004;
	pub const FLAG_SAMPLE_COMPOSITION_TIME_OFFSET: u32 = 0x000800;
	pub const FLAG_SAMPLE_DURATION: u32 = 0x000100;
	pub const FLAG_SAMPLE_FLAGS: u32 = 0x000400;
	pub const FLAG_SAMPLE_SIZE: u32 = 0x000200;
}

impl BoxType for Trun {
	const NAME: [u8; 4] = *b"trun";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let header = FullBoxHeader::demux(header, &mut reader)?;

		let sample_count = reader.read_u32::<BigEndian>()?;

		let data_offset = if header.flags & Self::FLAG_DATA_OFFSET != 0 {
			Some(reader.read_i32::<BigEndian>()?)
		} else {
			None
		};

		let first_sample_flags = if header.flags & Self::FLAG_FIRST_SAMPLE_FLAGS != 0 {
			Some(reader.read_u32::<BigEndian>()?.into())
		} else {
			None
		};

		let mut samples = Vec::with_capacity(sample_count as usize);
		for _ in 0..sample_count {
			let duration = if header.flags & Self::FLAG_SAMPLE_DURATION != 0 {
				Some(reader.read_u32::<BigEndian>()?)
			} else {
				None
			};

			let size = if header.flags & Self::FLAG_SAMPLE_SIZE != 0 {
				Some(reader.read_u32::<BigEndian>()?)
			} else {
				None
			};

			let flags = if header.flags & Self::FLAG_SAMPLE_FLAGS != 0 {
				Some(reader.read_u32::<BigEndian>()?.into())
			} else {
				None
			};

			let composition_time_offset = if header.flags & Self::FLAG_SAMPLE_COMPOSITION_TIME_OFFSET != 0 {
				if header.version == 1 {
					Some(reader.read_i32::<BigEndian>()? as i64)
				} else {
					Some(reader.read_u32::<BigEndian>()? as i64)
				}
			} else {
				None
			};

			samples.push(TrunSample {
				duration,
				size,
				flags,
				composition_time_offset,
			});
		}

		Ok(Self {
			header,
			data_offset,
			first_sample_flags,
			samples,
		})
	}

	fn primitive_size(&self) -> u64 {
		self.header.size()
        + 4 // sample_count
        + if self.header.flags & Self::FLAG_DATA_OFFSET != 0 { 4 } else { 0 }
        + if self.header.flags & Self::FLAG_FIRST_SAMPLE_FLAGS != 0 { 4 } else { 0 }
        + self.samples.iter().map(|_| {
            (if self.header.flags & Self::FLAG_SAMPLE_DURATION != 0 { 4 } else { 0 })
            + (if self.header.flags & Self::FLAG_SAMPLE_SIZE != 0 { 4 } else { 0 })
            + (if self.header.flags & Self::FLAG_SAMPLE_FLAGS != 0 { 4 } else { 0 })
            + (if self.header.flags & Self::FLAG_SAMPLE_COMPOSITION_TIME_OFFSET != 0 {
                4
            } else {
                0
            })
        }).sum::<u64>()
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.header.mux(writer)?;

		writer.write_u32::<BigEndian>(self.samples.len() as u32)?;

		if let Some(data_offset) = self.data_offset {
			writer.write_i32::<BigEndian>(data_offset)?;
		}

		if let Some(first_sample_flags) = self.first_sample_flags {
			writer.write_u32::<BigEndian>(first_sample_flags.into())?;
		}

		for sample in &self.samples {
			if let Some(duration) = sample.duration {
				writer.write_u32::<BigEndian>(duration)?;
			}

			if let Some(size) = sample.size {
				writer.write_u32::<BigEndian>(size)?;
			}

			if let Some(flags) = sample.flags {
				writer.write_u32::<BigEndian>(flags.into())?;
			}

			if let Some(composition_time_offset) = sample.composition_time_offset {
				if self.header.version == 1 {
					writer.write_i32::<BigEndian>(composition_time_offset as i32)?;
				} else {
					writer.write_u32::<BigEndian>(composition_time_offset as u32)?;
				}
			}
		}

		Ok(())
	}

	fn validate(&self) -> io::Result<()> {
		if self.header.version > 1 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "trun version must be 0 or 1"));
		}

		if self.header.flags & Self::FLAG_SAMPLE_COMPOSITION_TIME_OFFSET == 0 && self.header.version == 1 {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"trun version must be 0 if sample composition time offset is not present",
			));
		}

		if self.header.flags & Self::FLAG_DATA_OFFSET != 0 && self.data_offset.is_none() {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"trun data offset is present but not set",
			));
		} else if self.header.flags & Self::FLAG_DATA_OFFSET == 0 && self.data_offset.is_some() {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"trun data offset is not present but set",
			));
		}

		if self.header.flags & Self::FLAG_FIRST_SAMPLE_FLAGS != 0 && self.first_sample_flags.is_none() {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"trun first sample flags is present but not set",
			));
		} else if self.header.flags & Self::FLAG_FIRST_SAMPLE_FLAGS == 0 && self.first_sample_flags.is_some() {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"trun first sample flags is not present but set",
			));
		}

		if self.header.flags & Self::FLAG_SAMPLE_DURATION != 0 && self.samples.iter().any(|s| s.duration.is_none()) {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"trun sample duration is present but not set",
			));
		} else if self.header.flags & Self::FLAG_SAMPLE_DURATION == 0 && self.samples.iter().any(|s| s.duration.is_some()) {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"trun sample duration is not present but set",
			));
		}

		if self.header.flags & Self::FLAG_SAMPLE_SIZE != 0 && self.samples.iter().any(|s| s.size.is_none()) {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"trun sample size is present but not set",
			));
		} else if self.header.flags & Self::FLAG_SAMPLE_SIZE == 0 && self.samples.iter().any(|s| s.size.is_some()) {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"trun sample size is not present but set",
			));
		}

		if self.header.flags & Self::FLAG_SAMPLE_FLAGS != 0 && self.samples.iter().any(|s| s.flags.is_none()) {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"trun sample flags is present but not set",
			));
		} else if self.header.flags & Self::FLAG_SAMPLE_FLAGS == 0 && self.samples.iter().any(|s| s.flags.is_some()) {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"trun sample flags is not present but set",
			));
		}

		if self.header.flags & Self::FLAG_SAMPLE_COMPOSITION_TIME_OFFSET != 0
			&& self.samples.iter().any(|s| s.composition_time_offset.is_none())
		{
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"trun sample composition time offset is present but not set",
			));
		} else if self.header.flags & Self::FLAG_SAMPLE_COMPOSITION_TIME_OFFSET == 0
			&& self.samples.iter().any(|s| s.composition_time_offset.is_some())
		{
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"trun sample composition time offset is not present but set",
			));
		}

		if self.header.flags & Self::FLAG_SAMPLE_COMPOSITION_TIME_OFFSET != 0 {
			if self.header.version == 1
				&& self
					.samples
					.iter()
					.any(|s| s.composition_time_offset.unwrap() > i32::MAX as i64)
			{
				return Err(io::Error::new(
					io::ErrorKind::InvalidData,
					"trun sample composition time offset cannot be larger than i32::MAX",
				));
			} else if self.header.version == 0
				&& self
					.samples
					.iter()
					.any(|s| s.composition_time_offset.unwrap() > u32::MAX as i64)
			{
				return Err(io::Error::new(
					io::ErrorKind::InvalidData,
					"trun sample composition time offset cannot be larger than u32::MAX",
				));
			}
		}

		if let Some(first_sample_flags) = self.first_sample_flags {
			first_sample_flags.validate()?;
		}

		for sample in &self.samples {
			if let Some(flags) = sample.flags {
				flags.validate()?;
			}
		}

		Ok(())
	}
}
