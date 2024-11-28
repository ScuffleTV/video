use std::fmt::Debug;
use std::io::{
	Read, Write, {self},
};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;

use super::clap::Clap;
use super::colr::Colr;
use super::pasp::Pasp;
use crate::boxes::header::{BoxHeader, FullBoxHeader};
use crate::boxes::traits::BoxType;
use crate::boxes::DynBox;

#[derive(Debug, Clone, PartialEq)]
/// Sample Description Box
/// ISO/IEC 14496-12:2022(E) - 8.5.2
pub struct Stsd {
	pub header: FullBoxHeader,
	pub entries: Vec<DynBox>,
}

impl Stsd {
	pub fn new(entries: Vec<DynBox>) -> Self {
		Self {
			header: FullBoxHeader::new(Self::NAME, 0, 0),
			entries,
		}
	}

	pub fn get_codecs(&self) -> impl Iterator<Item = String> + '_ {
		self.entries.iter().filter_map(|e| match e {
			DynBox::Av01(av01) => av01.codec().ok().map(|c| c.to_string()),
			DynBox::Avc1(avc1) => avc1.codec().ok().map(|c| c.to_string()),
			DynBox::Hev1(hev1) => hev1.codec().ok().map(|c| c.to_string()),
			DynBox::Opus(opus) => opus.codec().ok().map(|c| c.to_string()),
			DynBox::Mp4a(mp4a) => mp4a.codec().ok().map(|c| c.to_string()),
			_ => None,
		})
	}

	pub fn is_audio(&self) -> bool {
		self.entries.iter().any(|e| matches!(e, DynBox::Mp4a(_) | DynBox::Opus(_)))
	}

	pub fn is_video(&self) -> bool {
		self.entries
			.iter()
			.any(|e| matches!(e, DynBox::Av01(_) | DynBox::Avc1(_) | DynBox::Hev1(_)))
	}
}

#[derive(Debug, Clone, PartialEq)]
/// Sample Entry Box
/// Contains a template field for the Type of Sample Entry
/// ISO/IEC 14496-12:2022(E) - 8.5.2.2
pub struct SampleEntry<T: SampleEntryExtension> {
	pub reserved: [u8; 6],
	pub data_reference_index: u16,
	pub extension: T,
}

impl<T: SampleEntryExtension> SampleEntry<T> {
	pub fn new(extension: T) -> Self {
		Self {
			reserved: [0; 6],
			data_reference_index: 1,
			extension,
		}
	}
}

pub trait SampleEntryExtension: Debug + Clone + PartialEq {
	fn demux<R: Read>(reader: &mut R) -> io::Result<Self>
	where
		Self: Sized;

	fn size(&self) -> u64;

	fn mux<W: Write>(&self, writer: &mut W) -> io::Result<()>;

	fn validate(&self) -> io::Result<()> {
		Ok(())
	}
}

impl<T: SampleEntryExtension> SampleEntry<T> {
	pub fn demux<R: Read>(reader: &mut R) -> io::Result<Self> {
		let mut reserved = [0; 6];
		reader.read_exact(&mut reserved)?;

		let data_reference_index = reader.read_u16::<BigEndian>()?;

		Ok(Self {
			reserved,
			data_reference_index,
			extension: T::demux(reader)?,
		})
	}

	pub fn size(&self) -> u64 {
		6 // reserved
        + 2 // data_reference_index
        + self.extension.size()
	}

	pub fn mux<W: Write>(&self, writer: &mut W) -> io::Result<()> {
		self.validate()?;

		writer.write_all(&self.reserved)?;
		writer.write_u16::<BigEndian>(self.data_reference_index)?;
		self.extension.mux(writer)?;

		Ok(())
	}

	pub fn validate(&self) -> io::Result<()> {
		if self.reserved != [0; 6] {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"sample entry reserved field must be 0",
			));
		}

		self.extension.validate()?;

		Ok(())
	}
}

#[derive(Debug, Clone, PartialEq)]
/// Audio Sample Entry Contents
/// ISO/IEC 14496-12:2022(E) - 12.2.3.2
pub struct AudioSampleEntry {
	pub reserved: [u32; 2],
	pub channel_count: u16,
	pub sample_size: u16,
	pub pre_defined: u16,
	pub reserved2: u16,
	pub sample_rate: u32,
}

impl AudioSampleEntry {
	pub fn new(channel_count: u16, sample_size: u16, sample_rate: u32) -> Self {
		Self {
			reserved: [0, 0],
			channel_count,
			sample_size,
			pre_defined: 0,
			reserved2: 0,
			sample_rate,
		}
	}
}

impl SampleEntryExtension for AudioSampleEntry {
	fn demux<T: io::Read>(reader: &mut T) -> io::Result<Self> {
		let reserved = [reader.read_u32::<BigEndian>()?, reader.read_u32::<BigEndian>()?];

		let channel_count = reader.read_u16::<BigEndian>()?;
		let sample_size = reader.read_u16::<BigEndian>()?;
		let pre_defined = reader.read_u16::<BigEndian>()?;
		let reserved2 = reader.read_u16::<BigEndian>()?;
		let sample_rate = reader.read_u32::<BigEndian>()? >> 16;

		Ok(Self {
			reserved,
			channel_count,
			sample_size,
			pre_defined,
			reserved2,
			sample_rate,
		})
	}

	fn size(&self) -> u64 {
		4 // reserved[0]
        + 4 // reserved[1]
        + 2 // channel_count
        + 2 // sample_size
        + 2 // pre_defined
        + 2 // reserved2
        + 4 // sample_rate
	}

	fn mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		writer.write_u32::<BigEndian>(self.reserved[0])?;
		writer.write_u32::<BigEndian>(self.reserved[1])?;
		writer.write_u16::<BigEndian>(self.channel_count)?;
		writer.write_u16::<BigEndian>(self.sample_size)?;
		writer.write_u16::<BigEndian>(self.pre_defined)?;
		writer.write_u16::<BigEndian>(self.reserved2)?;
		writer.write_u32::<BigEndian>(self.sample_rate << 16)?;

		Ok(())
	}

	fn validate(&self) -> io::Result<()> {
		if self.reserved != [0, 0] {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "reserved field must be 0"));
		}

		if self.pre_defined != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "pre_defined field must be 0"));
		}

		if self.reserved2 != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "reserved2 field must be 0"));
		}

		Ok(())
	}
}

#[derive(Debug, Clone, PartialEq)]
/// Visual Sample Entry Contents
/// ISO/IEC 14496-12:2022(E) - 12.1.3.2
pub struct VisualSampleEntry {
	pub pre_defined: u16,
	pub reserved: u16,
	pub pre_defined2: [u32; 3],
	pub width: u16,
	pub height: u16,
	pub horizresolution: u32,
	pub vertresolution: u32,
	pub reserved2: u32,
	pub frame_count: u16,
	pub compressorname: [u8; 32],
	pub depth: u16,
	pub pre_defined3: i16,
	pub clap: Option<Clap>,
	pub colr: Option<Colr>,
	pub pasp: Option<Pasp>,
}

impl VisualSampleEntry {
	pub fn new(width: u16, height: u16, colr: Option<Colr>) -> Self {
		Self {
			pre_defined: 0,
			reserved: 0,
			pre_defined2: [0, 0, 0],
			width,
			height,
			horizresolution: 0x00480000,
			vertresolution: 0x00480000,
			reserved2: 0,
			frame_count: 1,
			compressorname: [0; 32],
			depth: 0x0018,
			pre_defined3: -1,
			clap: None,
			colr,
			pasp: Some(Pasp::new()),
		}
	}
}

impl SampleEntryExtension for VisualSampleEntry {
	fn demux<T: io::Read>(reader: &mut T) -> io::Result<Self> {
		let pre_defined = reader.read_u16::<BigEndian>()?;
		let reserved = reader.read_u16::<BigEndian>()?;
		let pre_defined2 = [
			reader.read_u32::<BigEndian>()?,
			reader.read_u32::<BigEndian>()?,
			reader.read_u32::<BigEndian>()?,
		];
		let width = reader.read_u16::<BigEndian>()?;
		let height = reader.read_u16::<BigEndian>()?;
		let horizresolution = reader.read_u32::<BigEndian>()?;
		let vertresolution = reader.read_u32::<BigEndian>()?;
		let reserved2 = reader.read_u32::<BigEndian>()?;
		let frame_count = reader.read_u16::<BigEndian>()?;
		let mut compressorname = [0; 32];
		reader.read_exact(&mut compressorname)?;
		let depth = reader.read_u16::<BigEndian>()?;
		let pre_defined3 = reader.read_i16::<BigEndian>()?;

		Ok(Self {
			pre_defined,
			reserved,
			pre_defined2,
			width,
			height,
			horizresolution,
			vertresolution,
			reserved2,
			frame_count,
			compressorname,
			depth,
			pre_defined3,
			colr: None,
			clap: None,
			pasp: None,
		})
	}

	fn size(&self) -> u64 {
		2 // pre_defined
        + 2 // reserved
        + 4 // pre_defined2[0]
        + 4 // pre_defined2[1]
        + 4 // pre_defined2[2]
        + 2 // width
        + 2 // height
        + 4 // horizresolution
        + 4 // vertresolution
        + 4 // reserved2
        + 2 // frame_count
        + 32 // compressorname
        + 2 // depth
        + 2 // pre_defined3
        + self.clap.as_ref().map_or(0, |clap| clap.size())
        + self.pasp.as_ref().map_or(0, |pasp| pasp.size())
        + self.colr.as_ref().map_or(0, |colr| colr.size())
	}

	fn mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		writer.write_u16::<BigEndian>(self.pre_defined).unwrap();
		writer.write_u16::<BigEndian>(self.reserved).unwrap();
		writer.write_u32::<BigEndian>(self.pre_defined2[0]).unwrap();
		writer.write_u32::<BigEndian>(self.pre_defined2[1]).unwrap();
		writer.write_u32::<BigEndian>(self.pre_defined2[2]).unwrap();
		writer.write_u16::<BigEndian>(self.width).unwrap();
		writer.write_u16::<BigEndian>(self.height).unwrap();
		writer.write_u32::<BigEndian>(self.horizresolution).unwrap();
		writer.write_u32::<BigEndian>(self.vertresolution).unwrap();
		writer.write_u32::<BigEndian>(self.reserved2).unwrap();
		writer.write_u16::<BigEndian>(self.frame_count).unwrap();
		writer.write_all(&self.compressorname).unwrap();
		writer.write_u16::<BigEndian>(self.depth).unwrap();
		writer.write_i16::<BigEndian>(self.pre_defined3).unwrap();
		if let Some(clap) = &self.clap {
			clap.mux(writer).unwrap();
		}
		if let Some(pasp) = &self.pasp {
			pasp.mux(writer).unwrap();
		}
		if let Some(colr) = &self.colr {
			colr.mux(writer).unwrap();
		}
		Ok(())
	}

	fn validate(&self) -> io::Result<()> {
		if self.pre_defined != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "pre_defined field must be 0"));
		}

		if self.reserved != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "reserved field must be 0"));
		}

		if self.pre_defined2 != [0, 0, 0] {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "pre_defined2 field must be 0"));
		}

		if self.reserved2 != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "reserved2 field must be 0"));
		}

		if self.pre_defined3 != -1 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "pre_defined3 field must be -1"));
		}

		Ok(())
	}
}

impl BoxType for Stsd {
	const NAME: [u8; 4] = *b"stsd";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let header = FullBoxHeader::demux(header, &mut reader)?;

		let entry_count = reader.read_u32::<BigEndian>()?;
		let mut entries = Vec::with_capacity(entry_count as usize);

		for _ in 0..entry_count {
			let entry = DynBox::demux(&mut reader)?;
			entries.push(entry);
		}

		Ok(Self { header, entries })
	}

	fn primitive_size(&self) -> u64 {
		self.header.size()
            + 4 // entry_count
            + self.entries.iter().map(|entry| entry.size()).sum::<u64>()
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.header.mux(writer)?;
		writer.write_u32::<BigEndian>(self.entries.len() as u32)?;
		for entry in &self.entries {
			entry.mux(writer)?;
		}
		Ok(())
	}

	fn validate(&self) -> io::Result<()> {
		if self.header.flags != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "stsd flags must be 0"));
		}

		if self.header.version != 0 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "stsd version must be 0"));
		}

		Ok(())
	}
}
