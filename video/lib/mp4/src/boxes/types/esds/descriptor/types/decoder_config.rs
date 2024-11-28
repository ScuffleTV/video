use std::io;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::{Buf, Bytes};

use super::decoder_specific_info::DecoderSpecificInfoDescriptor;
use crate::boxes::types::esds::descriptor::header::{DescriptorHeader, DescriptorTag};
use crate::boxes::types::esds::descriptor::traits::DescriptorType;
use crate::boxes::types::esds::descriptor::DynDescriptor;

#[derive(Debug, Clone, PartialEq)]
/// Decoder Config Descriptor
/// ISO/IEC 14496-1:2010(E) - 7.2.6.6
pub struct DecoderConfigDescriptor {
	pub header: DescriptorHeader,
	pub object_type_indication: u8,
	pub stream_type: u8,     // 6 bits
	pub up_stream: bool,     // 1 bit
	pub reserved: u8,        // 1 bit
	pub buffer_size_db: u32, // 3 bytes
	pub max_bitrate: u32,
	pub avg_bitrate: u32,
	pub decoder_specific_info: Option<DecoderSpecificInfoDescriptor>,
	pub unknown: Vec<DynDescriptor>,
}

impl DecoderConfigDescriptor {
	pub fn new(
		object_type_indication: u8,
		stream_type: u8,
		max_bitrate: u32,
		avg_bitrate: u32,
		decoder_specific_info: Option<DecoderSpecificInfoDescriptor>,
	) -> Self {
		Self {
			header: DescriptorHeader::new(Self::TAG),
			object_type_indication,
			stream_type,
			up_stream: false,
			reserved: 1,
			buffer_size_db: 0,
			max_bitrate,
			avg_bitrate,
			decoder_specific_info,
			unknown: Vec::new(),
		}
	}
}

impl DescriptorType for DecoderConfigDescriptor {
	const TAG: DescriptorTag = DescriptorTag::DecoderConfigDescrTag;

	fn demux(header: DescriptorHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let object_type_indication = reader.read_u8()?;
		let byte = reader.read_u8()?;
		let stream_type = byte >> 2;
		let up_stream = (byte & 0b00000010) != 0;
		let reserved = byte & 0b00000001;
		let buffer_size_db = reader.read_u24::<BigEndian>()?;
		let max_bitrate = reader.read_u32::<BigEndian>()?;
		let avg_bitrate = reader.read_u32::<BigEndian>()?;

		let mut decoder_specific_info = None;
		let mut unknown = Vec::new();

		while reader.has_remaining() {
			let descriptor = DynDescriptor::demux(&mut reader)?;
			match descriptor {
				DynDescriptor::DecoderSpecificInfo(desc) => {
					decoder_specific_info = Some(desc);
				}
				_ => unknown.push(descriptor),
			}
		}

		Ok(Self {
			header,
			object_type_indication,
			stream_type,
			up_stream,
			reserved,
			buffer_size_db,
			max_bitrate,
			avg_bitrate,
			decoder_specific_info,
			unknown,
		})
	}

	fn primitive_size(&self) -> u64 {
		1 + 1
			+ 3 + 4 + 4 + self.decoder_specific_info.as_ref().map(|d| d.size()).unwrap_or(0)
			+ self.unknown.iter().map(|d| d.size()).sum::<u64>()
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		writer.write_u8(self.object_type_indication)?;
		let byte = (self.stream_type << 2) | (self.up_stream as u8) << 1 | self.reserved;
		writer.write_u8(byte)?;
		writer.write_u24::<BigEndian>(self.buffer_size_db)?;
		writer.write_u32::<BigEndian>(self.max_bitrate)?;
		writer.write_u32::<BigEndian>(self.avg_bitrate)?;
		if let Some(decoder_specific_info) = &self.decoder_specific_info {
			decoder_specific_info.mux(writer)?;
		}
		for descriptor in &self.unknown {
			descriptor.mux(writer)?;
		}

		Ok(())
	}
}
