use std::io::{
	Read, Write, {self},
};

use byteorder::{BigEndian, LittleEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;
use bytesio::bit_reader::BitReader;
use bytesio::bit_writer::BitWriter;

#[derive(Debug, Clone, PartialEq)]
/// HEVC Decoder Configuration Record
/// ISO/IEC 14496-15:2022(E) - 8.3.2.1
pub struct HEVCDecoderConfigurationRecord {
	pub configuration_version: u8,
	pub general_profile_space: u8,
	pub general_tier_flag: bool,
	pub general_profile_idc: u8,
	pub general_profile_compatibility_flags: u32,
	pub general_constraint_indicator_flags: u64,
	pub general_level_idc: u8,
	pub min_spatial_segmentation_idc: u16,
	pub parallelism_type: u8,
	pub chroma_format_idc: u8,
	pub bit_depth_luma_minus8: u8,
	pub bit_depth_chroma_minus8: u8,
	pub avg_frame_rate: u16,
	pub constant_frame_rate: u8,
	pub num_temporal_layers: u8,
	pub temporal_id_nested: bool,
	pub length_size_minus_one: u8,
	pub arrays: Vec<NaluArray>,
}

#[derive(Debug, Clone, PartialEq)]
/// Nalu Array Structure
/// ISO/IEC 14496-15:2022(E) - 8.3.2.1
pub struct NaluArray {
	pub array_completeness: bool,
	pub nal_unit_type: NaluType,
	pub nalus: Vec<Bytes>,
}

#[derive(Debug, Clone, PartialEq, Copy)]
/// Nalu Type
/// ISO/IEC 23008-2:2020(E) - 7.4.2.2 (Table 7-1)
pub enum NaluType {
	Vps,
	Pps,
	Sps,
	Unknown(u8),
}

impl From<u8> for NaluType {
	fn from(value: u8) -> Self {
		match value {
			32 => NaluType::Vps,
			33 => NaluType::Sps,
			34 => NaluType::Pps,
			_ => NaluType::Unknown(value),
		}
	}
}

impl From<NaluType> for u8 {
	fn from(value: NaluType) -> Self {
		match value {
			NaluType::Vps => 32,
			NaluType::Sps => 33,
			NaluType::Pps => 34,
			NaluType::Unknown(value) => value,
		}
	}
}

impl HEVCDecoderConfigurationRecord {
	pub fn demux(data: &mut io::Cursor<Bytes>) -> io::Result<Self> {
		let mut bit_reader = BitReader::new(data);

		let configuration_version = bit_reader.read_u8()?;
		let general_profile_space = bit_reader.read_bits(2)? as u8;
		let general_tier_flag = bit_reader.read_bit()?;
		let general_profile_idc = bit_reader.read_bits(5)? as u8;
		let general_profile_compatibility_flags = bit_reader.read_u32::<LittleEndian>()?;
		let general_constraint_indicator_flags = bit_reader.read_u48::<LittleEndian>()?;
		let general_level_idc = bit_reader.read_u8()?;

		bit_reader.seek_bits(4)?; // reserved_4bits
		let min_spatial_segmentation_idc = bit_reader.read_bits(12)? as u16;

		bit_reader.seek_bits(6)?; // reserved_6bits
		let parallelism_type = bit_reader.read_bits(2)? as u8;

		bit_reader.seek_bits(6)?; // reserved_6bits
		let chroma_format_idc = bit_reader.read_bits(2)? as u8;

		bit_reader.seek_bits(5)?; // reserved_5bits
		let bit_depth_luma_minus8 = bit_reader.read_bits(3)? as u8;

		bit_reader.seek_bits(5)?; // reserved_5bits
		let bit_depth_chroma_minus8 = bit_reader.read_bits(3)? as u8;

		let avg_frame_rate = bit_reader.read_u16::<BigEndian>()?;
		let constant_frame_rate = bit_reader.read_bits(2)? as u8;
		let num_temporal_layers = bit_reader.read_bits(3)? as u8;
		let temporal_id_nested = bit_reader.read_bit()?;
		let length_size_minus_one = bit_reader.read_bits(2)? as u8;

		let num_of_arrays = bit_reader.read_u8()?;

		let mut arrays = Vec::with_capacity(num_of_arrays as usize);

		for _ in 0..num_of_arrays {
			let array_completeness = bit_reader.read_bit()?;
			bit_reader.seek_bits(1)?; // reserved

			let nal_unit_type = bit_reader.read_bits(6)? as u8;

			let num_nalus = bit_reader.read_u16::<BigEndian>()?;

			let mut nalus = Vec::with_capacity(num_nalus as usize);

			for _ in 0..num_nalus {
				let nal_unit_length = bit_reader.read_u16::<BigEndian>()?;
				let mut data = vec![0; nal_unit_length as usize];
				bit_reader.read_exact(&mut data)?;
				nalus.push(data.into());
			}

			arrays.push(NaluArray {
				array_completeness,
				nal_unit_type: nal_unit_type.into(),
				nalus,
			});
		}

		Ok(HEVCDecoderConfigurationRecord {
			configuration_version,
			general_profile_space,
			general_tier_flag,
			general_profile_idc,
			general_profile_compatibility_flags,
			general_constraint_indicator_flags,
			general_level_idc,
			min_spatial_segmentation_idc,
			parallelism_type,
			chroma_format_idc,
			bit_depth_luma_minus8,
			bit_depth_chroma_minus8,
			avg_frame_rate,
			constant_frame_rate,
			num_temporal_layers,
			temporal_id_nested,
			length_size_minus_one,
			arrays,
		})
	}

	pub fn size(&self) -> u64 {
		1 // configuration_version
        + 1 // general_profile_space, general_tier_flag, general_profile_idc
        + 4 // general_profile_compatibility_flags
        + 6 // general_constraint_indicator_flags
        + 1 // general_level_idc
        + 2 // reserved_4bits, min_spatial_segmentation_idc
        + 1 // reserved_6bits, parallelism_type
        + 1 // reserved_6bits, chroma_format_idc
        + 1 // reserved_5bits, bit_depth_luma_minus8
        + 1 // reserved_5bits, bit_depth_chroma_minus8
        + 2 // avg_frame_rate
        + 1 // constant_frame_rate, num_temporal_layers, temporal_id_nested, length_size_minus_one
        + 1 // num_of_arrays
        + self.arrays.iter().map(|array| {
            1 // array_completeness, reserved, nal_unit_type
            + 2 // num_nalus
            + array.nalus.iter().map(|nalu| {
                2 // nal_unit_length
                + nalu.len() as u64 // nal_unit
            }).sum::<u64>()
        }).sum::<u64>()
	}

	pub fn mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		let mut bit_writer = BitWriter::default();

		bit_writer.write_u8(self.configuration_version)?;
		bit_writer.write_bits(self.general_profile_space as u64, 2)?;
		bit_writer.write_bit(self.general_tier_flag)?;
		bit_writer.write_bits(self.general_profile_idc as u64, 5)?;
		bit_writer.write_u32::<LittleEndian>(self.general_profile_compatibility_flags)?;
		bit_writer.write_u48::<LittleEndian>(self.general_constraint_indicator_flags)?;
		bit_writer.write_u8(self.general_level_idc)?;

		bit_writer.write_bits(0b1111, 4)?; // reserved_4bits
		bit_writer.write_bits(self.min_spatial_segmentation_idc as u64, 12)?;

		bit_writer.write_bits(0b111111, 6)?; // reserved_6bits
		bit_writer.write_bits(self.parallelism_type as u64, 2)?;

		bit_writer.write_bits(0b111111, 6)?; // reserved_6bits
		bit_writer.write_bits(self.chroma_format_idc as u64, 2)?;

		bit_writer.write_bits(0b11111, 5)?; // reserved_5bits
		bit_writer.write_bits(self.bit_depth_luma_minus8 as u64, 3)?;

		bit_writer.write_bits(0b11111, 5)?; // reserved_5bits
		bit_writer.write_bits(self.bit_depth_chroma_minus8 as u64, 3)?;

		bit_writer.write_u16::<BigEndian>(self.avg_frame_rate)?;
		bit_writer.write_bits(self.constant_frame_rate as u64, 2)?;

		bit_writer.write_bits(self.num_temporal_layers as u64, 3)?;
		bit_writer.write_bit(self.temporal_id_nested)?;
		bit_writer.write_bits(self.length_size_minus_one as u64, 2)?;

		bit_writer.write_u8(self.arrays.len() as u8)?;
		for array in &self.arrays {
			bit_writer.write_bit(array.array_completeness)?;
			bit_writer.write_bits(0b0, 1)?; // reserved
			bit_writer.write_bits(u8::from(array.nal_unit_type) as u64, 6)?;

			bit_writer.write_u16::<BigEndian>(array.nalus.len() as u16)?;

			for nalu in &array.nalus {
				bit_writer.write_u16::<BigEndian>(nalu.len() as u16)?;
				bit_writer.write_all(nalu)?;
			}
		}

		writer.write_all(&bit_writer.into_inner())?;

		Ok(())
	}
}
