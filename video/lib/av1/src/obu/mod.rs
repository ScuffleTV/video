use std::io::{
	Read, {self},
};

use bytes::Bytes;
use bytesio::bit_reader::BitReader;

pub mod seq;

#[derive(Debug, Clone, PartialEq)]
/// OBU Header
/// AV1-Spec-2 - 5.3.2
pub struct ObuHeader {
	pub obu_type: ObuType,
	pub extension_flag: bool,
	pub has_size_field: bool,
	pub extension_header: Option<ObuHeaderExtension>,
}

#[derive(Debug, Clone, PartialEq)]
/// Obu Header Extension
/// AV1-Spec-2 - 5.3.3
pub struct ObuHeaderExtension {
	pub temporal_id: u8,
	pub spatial_id: u8,
}

impl ObuHeader {
	pub fn parse<T: io::Read>(bit_reader: &mut BitReader<T>) -> io::Result<(Self, Bytes)> {
		let forbidden_bit = bit_reader.read_bit()?;
		if forbidden_bit {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "obu_forbidden_bit is not 0"));
		}

		let obu_type = bit_reader.read_bits(4)?;
		let extension_flag = bit_reader.read_bit()?;
		let has_size_field = bit_reader.read_bit()?;

		let reserved_1bit = bit_reader.read_bit()?;
		if reserved_1bit {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "obu_reserved_1bit is not 0"));
		}

		let extension_header = if extension_flag {
			let temporal_id = bit_reader.read_bits(3)?;
			let spatial_id = bit_reader.read_bits(2)?;
			bit_reader.read_bits(3)?; // reserved_3bits
			Some(ObuHeaderExtension {
				temporal_id: temporal_id as u8,
				spatial_id: spatial_id as u8,
			})
		} else {
			None
		};

		let size = if has_size_field {
			// obu_size
			read_leb128(bit_reader)?
		} else {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "obu_size is not present"));
		};

		let mut data = vec![0; size as usize];
		bit_reader.read_exact(&mut data)?;

		Ok((
			ObuHeader {
				obu_type: ObuType::from(obu_type as u8),
				extension_flag,
				has_size_field,
				extension_header,
			},
			Bytes::from(data),
		))
	}
}

#[derive(Debug, Clone, PartialEq)]
/// OBU Type
/// AV1-Spec-2 - 6.2.2
pub enum ObuType {
	SequenceHeader,
	TemporalDelimiter,
	FrameHeader,
	TileGroup,
	Metadata,
	Frame,
	RedundantFrameHeader,
	TileList,
	Padding,
	Reserved(u8),
}

impl From<u8> for ObuType {
	fn from(value: u8) -> Self {
		match value {
			1 => ObuType::SequenceHeader,
			2 => ObuType::TemporalDelimiter,
			3 => ObuType::FrameHeader,
			4 => ObuType::TileGroup,
			5 => ObuType::Metadata,
			6 => ObuType::Frame,
			7 => ObuType::RedundantFrameHeader,
			8 => ObuType::TileList,
			15 => ObuType::Padding,
			_ => ObuType::Reserved(value),
		}
	}
}

impl From<ObuType> for u8 {
	fn from(value: ObuType) -> Self {
		match value {
			ObuType::SequenceHeader => 1,
			ObuType::TemporalDelimiter => 2,
			ObuType::FrameHeader => 3,
			ObuType::TileGroup => 4,
			ObuType::Metadata => 5,
			ObuType::Frame => 6,
			ObuType::RedundantFrameHeader => 7,
			ObuType::TileList => 8,
			ObuType::Padding => 15,
			ObuType::Reserved(value) => value,
		}
	}
}

/// Read a little-endian variable-length integer.
/// AV1-Spec-2 - 4.10.5
fn read_leb128<T: io::Read>(reader: &mut BitReader<T>) -> io::Result<u64> {
	let mut result = 0;
	for i in 0..8 {
		let byte = reader.read_bits(8)?;
		result |= (byte & 0x7f) << (i * 7);
		if byte & 0x80 == 0 {
			break;
		}
	}
	Ok(result)
}

/// Read a variable-length unsigned integer.
/// AV1-Spec-2 - 4.10.3
fn read_uvlc<T: io::Read>(reader: &mut BitReader<T>) -> io::Result<u64> {
	let mut leading_zeros = 0;
	while !reader.read_bit()? {
		leading_zeros += 1;
	}

	if leading_zeros >= 32 {
		return Ok((1 << 32) - 1);
	}

	let value = reader.read_bits(leading_zeros)?;
	Ok(value + (1 << leading_zeros) - 1)
}
