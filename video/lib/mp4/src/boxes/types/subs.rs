use std::io;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;

use crate::boxes::header::{BoxHeader, FullBoxHeader};
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// Subsample Information Box
/// ISO/IEC 14496-12:2022(E) - 8.7.7
pub struct Subs {
	pub header: FullBoxHeader,

	pub entries: Vec<SubsEntry>,
}

#[derive(Debug, Clone, PartialEq)]
/// Subs box entry
pub struct SubsEntry {
	pub sample_delta: u32,
	pub subsamples: Vec<SubSampleEntry>,
}

#[derive(Debug, Clone, PartialEq)]
/// Sub Sample Entry
pub struct SubSampleEntry {
	pub subsample_size: u32,
	pub subsample_priority: u8,
	pub discardable: u8,
	pub codec_specific_parameters: u32,
}

impl BoxType for Subs {
	const NAME: [u8; 4] = *b"subs";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let header = FullBoxHeader::demux(header, &mut reader)?;

		let entry_count = reader.read_u32::<BigEndian>()?;
		let mut entries = Vec::with_capacity(entry_count as usize);

		for _ in 0..entry_count {
			let sample_delta = reader.read_u32::<BigEndian>()?;
			let subsample_count = reader.read_u16::<BigEndian>()?;
			let mut subsamples = Vec::with_capacity(subsample_count as usize);

			for _ in 0..subsample_count {
				let subsample_size = if header.version == 1 {
					reader.read_u32::<BigEndian>()?
				} else {
					reader.read_u16::<BigEndian>()? as u32
				};
				let subsample_priority = reader.read_u8()?;
				let discardable = reader.read_u8()?;
				let codec_specific_parameters = reader.read_u32::<BigEndian>()?;
				subsamples.push(SubSampleEntry {
					subsample_size,
					subsample_priority,
					discardable,
					codec_specific_parameters,
				});
			}

			entries.push(SubsEntry {
				sample_delta,
				subsamples,
			});
		}

		Ok(Self { header, entries })
	}

	fn primitive_size(&self) -> u64 {
		let size = self.header.size();
		let size = size + 4; // entry_count
		let size = size
			+ self
				.entries
				.iter()
				.map(|e| {
					let size = 4; // sample_delta
					let size = size + 2; // subsample_count

					size + e.subsamples.len() as u64
						* if self.header.version == 1 {
							4 + 1 + 1 + 4
						} else {
							2 + 1 + 1 + 4
						}
				})
				.sum::<u64>(); // entries
		size
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.header.mux(writer)?;

		writer.write_u32::<BigEndian>(self.entries.len() as u32)?;
		for entry in &self.entries {
			writer.write_u32::<BigEndian>(entry.sample_delta)?;
			writer.write_u16::<BigEndian>(entry.subsamples.len() as u16)?;
			for subsample in &entry.subsamples {
				if self.header.version == 1 {
					writer.write_u32::<BigEndian>(subsample.subsample_size)?;
				} else {
					writer.write_u16::<BigEndian>(subsample.subsample_size as u16)?;
				}
				writer.write_u8(subsample.subsample_priority)?;
				writer.write_u8(subsample.discardable)?;
				writer.write_u32::<BigEndian>(subsample.codec_specific_parameters)?;
			}
		}

		Ok(())
	}

	fn validate(&self) -> io::Result<()> {
		if self.header.version > 1 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "subs version must be 0 or 1"));
		}

		if self.header.version == 0 {
			for entry in &self.entries {
				for subsample in &entry.subsamples {
					if subsample.subsample_size > u16::MAX as u32 {
						return Err(io::Error::new(
							io::ErrorKind::InvalidData,
							"subs subsample_size must be less than 2^16",
						));
					}
				}
			}
		}

		Ok(())
	}
}
