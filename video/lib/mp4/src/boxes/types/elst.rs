use std::io;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;

use crate::boxes::header::{BoxHeader, FullBoxHeader};
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// Edit List Box
/// ISO/IEC 14496-12:2022(E) - 8.6.6
pub struct Elst {
	pub header: FullBoxHeader,
	pub entries: Vec<ElstEntry>,
}

impl Elst {
	pub fn new(entries: Vec<ElstEntry>) -> Self {
		Self {
			header: FullBoxHeader::new(Self::NAME, 0, 0),
			entries,
		}
	}
}

#[derive(Debug, Clone, PartialEq)]
/// Entry in the Edit List Box
pub struct ElstEntry {
	pub segment_duration: u64,
	pub media_time: i64,
	pub media_rate_integer: i16,
	pub media_rate_fraction: i16,
}

impl BoxType for Elst {
	const NAME: [u8; 4] = *b"elst";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let header = FullBoxHeader::demux(header, &mut reader)?;

		let entry_count = reader.read_u32::<BigEndian>()?;

		let mut entries = Vec::with_capacity(entry_count as usize);

		for _ in 0..entry_count {
			let (segment_duration, media_time, media_rate_integer, media_rate_fraction) = if header.version == 1 {
				(
					reader.read_u64::<BigEndian>()?, // segment_duration
					reader.read_i64::<BigEndian>()?, // media_time
					reader.read_i16::<BigEndian>()?, // media_rate_integer
					reader.read_i16::<BigEndian>()?, // media_rate_fraction
				)
			} else {
				(
					reader.read_u32::<BigEndian>()? as u64, // segment_duration
					reader.read_i32::<BigEndian>()? as i64, // media_time
					reader.read_i16::<BigEndian>()?,        // media_rate_integer
					reader.read_i16::<BigEndian>()?,        // media_rate_fraction
				)
			};

			entries.push(ElstEntry {
				segment_duration,
				media_time,
				media_rate_integer,
				media_rate_fraction,
			});
		}

		Ok(Self { header, entries })
	}

	fn primitive_size(&self) -> u64 {
		self.header.size()
        + 4 // entry_count
        + (self.entries.len() as u64) * if self.header.version == 1 {
            8 + 8 + 2 + 2 // segment_duration + media_time + media_rate_integer + media_rate_fraction
        } else {
            4 + 4 + 2 + 2 // segment_duration + media_time + media_rate_integer + media_rate_fraction
        }
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.header.mux(writer)?;

		writer.write_u32::<BigEndian>(self.entries.len() as u32)?;

		for entry in &self.entries {
			if self.header.version == 1 {
				writer.write_u64::<BigEndian>(entry.segment_duration)?;
				writer.write_i64::<BigEndian>(entry.media_time)?;
			} else {
				writer.write_u32::<BigEndian>(entry.segment_duration as u32)?;
				writer.write_i32::<BigEndian>(entry.media_time as i32)?;
			}

			writer.write_i16::<BigEndian>(entry.media_rate_integer)?;
			writer.write_i16::<BigEndian>(entry.media_rate_fraction)?;
		}

		Ok(())
	}

	fn validate(&self) -> io::Result<()> {
		if self.header.flags != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "elst: version 1 is not supported"));
		}

		if self.header.version > 1 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "elst: version must be 0 or 1"));
		}

		if self.header.version == 1 {
			for entry in &self.entries {
				if entry.segment_duration > u32::MAX as u64 {
					return Err(io::Error::new(
						io::ErrorKind::InvalidData,
						"elst: segment_duration must be u32",
					));
				}

				if entry.media_time > i32::MAX as i64 {
					return Err(io::Error::new(io::ErrorKind::InvalidData, "elst: media_time must be i32"));
				}
			}
		}

		Ok(())
	}
}
