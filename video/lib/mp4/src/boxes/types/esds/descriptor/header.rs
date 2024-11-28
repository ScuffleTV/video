use std::io;

use byteorder::ReadBytesExt;
use bytes::Bytes;
use bytesio::bytes_reader::BytesCursor;

#[derive(Debug, Clone, PartialEq)]
pub struct DescriptorHeader {
	pub tag: DescriptorTag,
}

impl DescriptorHeader {
	pub fn new(tag: DescriptorTag) -> Self {
		Self { tag }
	}

	pub fn parse(reader: &mut io::Cursor<Bytes>) -> Result<(Self, Bytes), io::Error> {
		let tag = reader.read_u8()?.into();

		let mut size = 0_u32;
		loop {
			let byte = reader.read_u8()?;
			size = (size << 7) | (byte & 0b01111111) as u32;
			if (byte & 0b10000000) == 0 {
				break;
			}
		}

		let data = reader.read_slice(size as usize)?;

		Ok((Self { tag }, data))
	}
}

#[derive(Debug, Clone, PartialEq, Copy, Eq)]
/// Descriptor Tags
/// ISO/IEC 14496-1:2010(E) - 7.2.2
pub enum DescriptorTag {
	ESDescrTag,
	DecoderConfigDescrTag,
	DecSpecificInfoTag,
	SLConfigDescrTag,
	Unknown(u8),
}

impl From<u8> for DescriptorTag {
	fn from(tag: u8) -> Self {
		match tag {
			0x03 => Self::ESDescrTag,
			0x04 => Self::DecoderConfigDescrTag,
			0x05 => Self::DecSpecificInfoTag,
			0x06 => Self::SLConfigDescrTag,
			_ => Self::Unknown(tag),
		}
	}
}

impl From<DescriptorTag> for u8 {
	fn from(tag: DescriptorTag) -> Self {
		match tag {
			DescriptorTag::ESDescrTag => 0x03,
			DescriptorTag::DecoderConfigDescrTag => 0x04,
			DescriptorTag::DecSpecificInfoTag => 0x05,
			DescriptorTag::SLConfigDescrTag => 0x06,
			DescriptorTag::Unknown(tag) => tag,
		}
	}
}
