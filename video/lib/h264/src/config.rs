use std::io::{
	Write, {self},
};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::{Buf, Bytes};
use bytesio::bit_writer::BitWriter;
use bytesio::bytes_reader::BytesCursor;

#[derive(Debug, Clone, PartialEq)]
/// AVC (H.264) Decoder Configuration Record
/// ISO/IEC 14496-15:2022(E) - 5.3.2.1.2
pub struct AVCDecoderConfigurationRecord {
	pub configuration_version: u8,
	pub profile_indication: u8,
	pub profile_compatibility: u8,
	pub level_indication: u8,
	pub length_size_minus_one: u8,
	pub sps: Vec<Bytes>,
	pub pps: Vec<Bytes>,
	pub extended_config: Option<AvccExtendedConfig>,
}

#[derive(Debug, Clone, PartialEq)]
/// AVC (H.264) Extended Configuration
/// ISO/IEC 14496-15:2022(E) - 5.3.2.1.2
pub struct AvccExtendedConfig {
	pub chroma_format: u8,
	pub bit_depth_luma_minus8: u8,
	pub bit_depth_chroma_minus8: u8,
	pub sequence_parameter_set_ext: Vec<Bytes>,
}

impl AVCDecoderConfigurationRecord {
	pub fn demux(reader: &mut io::Cursor<Bytes>) -> io::Result<Self> {
		let configuration_version = reader.read_u8()?;
		let profile_indication = reader.read_u8()?;
		let profile_compatibility = reader.read_u8()?;
		let level_indication = reader.read_u8()?;
		let length_size_minus_one = reader.read_u8()? & 0b00000011;
		let num_of_sequence_parameter_sets = reader.read_u8()? & 0b00011111;

		let mut sps = Vec::with_capacity(num_of_sequence_parameter_sets as usize);
		for _ in 0..num_of_sequence_parameter_sets {
			let sps_length = reader.read_u16::<BigEndian>()?;
			let sps_data = reader.read_slice(sps_length as usize)?;
			sps.push(sps_data);
		}

		let num_of_picture_parameter_sets = reader.read_u8()?;
		let mut pps = Vec::with_capacity(num_of_picture_parameter_sets as usize);
		for _ in 0..num_of_picture_parameter_sets {
			let pps_length = reader.read_u16::<BigEndian>()?;
			let pps_data = reader.read_slice(pps_length as usize)?;
			pps.push(pps_data);
		}

		// It turns out that sometimes the extended config is not present, even though
		// the avc_profile_indication is not 66, 77 or 88. We need to be lenient here on
		// decoding.
		let extended_config = match profile_indication {
			66 | 77 | 88 => None,
			_ => {
				if reader.has_remaining() {
					let chroma_format = reader.read_u8()? & 0b00000011; // 2 bits (6 bits reserved)
					let bit_depth_luma_minus8 = reader.read_u8()? & 0b00000111; // 3 bits (5 bits reserved)
					let bit_depth_chroma_minus8 = reader.read_u8()? & 0b00000111; // 3 bits (5 bits reserved)
					let number_of_sequence_parameter_set_ext = reader.read_u8()?; // 8 bits

					let mut sequence_parameter_set_ext = Vec::with_capacity(number_of_sequence_parameter_set_ext as usize);
					for _ in 0..number_of_sequence_parameter_set_ext {
						let sps_ext_length = reader.read_u16::<BigEndian>()?;
						let sps_ext_data = reader.read_slice(sps_ext_length as usize)?;
						sequence_parameter_set_ext.push(sps_ext_data);
					}

					Some(AvccExtendedConfig {
						chroma_format,
						bit_depth_luma_minus8,
						bit_depth_chroma_minus8,
						sequence_parameter_set_ext,
					})
				} else {
					// No extended config present even though avc_profile_indication is not 66, 77
					// or 88
					None
				}
			}
		};

		Ok(Self {
			configuration_version,
			profile_indication,
			profile_compatibility,
			level_indication,
			length_size_minus_one,
			sps,
			pps,
			extended_config,
		})
	}

	pub fn size(&self) -> u64 {
		1 // configuration_version
        + 1 // avc_profile_indication
        + 1 // profile_compatibility
        + 1 // avc_level_indication
        + 1 // length_size_minus_one
        + 1 // num_of_sequence_parameter_sets (5 bits reserved, 3 bits)
        + self.sps.iter().map(|sps| {
            2 // sps_length
            + sps.len() as u64
        }).sum::<u64>() // sps
        + 1 // num_of_picture_parameter_sets
        + self.pps.iter().map(|pps| {
            2 // pps_length
            + pps.len() as u64
        }).sum::<u64>() // pps
        + match &self.extended_config {
            Some(config) => {
                1 // chroma_format (6 bits reserved, 2 bits)
                + 1 // bit_depth_luma_minus8 (5 bits reserved, 3 bits)
                + 1 // bit_depth_chroma_minus8 (5 bits reserved, 3 bits)
                + 1 // number_of_sequence_parameter_set_ext
                + config.sequence_parameter_set_ext.iter().map(|sps_ext| {
                    2 // sps_ext_length
                    + sps_ext.len() as u64
                }).sum::<u64>() // sps_ext
            }
            None => 0,
        }
	}

	pub fn mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		let mut bit_writer = BitWriter::default();

		bit_writer.write_u8(self.configuration_version)?;
		bit_writer.write_u8(self.profile_indication)?;
		bit_writer.write_u8(self.profile_compatibility)?;
		bit_writer.write_u8(self.level_indication)?;
		bit_writer.write_bits(0b111111, 6)?;
		bit_writer.write_bits(self.length_size_minus_one as u64, 2)?;
		bit_writer.write_bits(0b111, 3)?;

		bit_writer.write_bits(self.sps.len() as u64, 5)?;
		for sps in &self.sps {
			bit_writer.write_u16::<BigEndian>(sps.len() as u16)?;
			bit_writer.write_all(sps)?;
		}

		bit_writer.write_bits(self.pps.len() as u64, 8)?;
		for pps in &self.pps {
			bit_writer.write_u16::<BigEndian>(pps.len() as u16)?;
			bit_writer.write_all(pps)?;
		}

		if let Some(config) = &self.extended_config {
			bit_writer.write_bits(0b111111, 6)?;
			bit_writer.write_bits(config.chroma_format as u64, 2)?;
			bit_writer.write_bits(0b11111, 5)?;
			bit_writer.write_bits(config.bit_depth_luma_minus8 as u64, 3)?;
			bit_writer.write_bits(0b11111, 5)?;
			bit_writer.write_bits(config.bit_depth_chroma_minus8 as u64, 3)?;

			bit_writer.write_bits(config.sequence_parameter_set_ext.len() as u64, 8)?;
			for sps_ext in &config.sequence_parameter_set_ext {
				bit_writer.write_u16::<BigEndian>(sps_ext.len() as u16)?;
				bit_writer.write_all(sps_ext)?;
			}
		}

		writer.write_all(&bit_writer.into_inner())?;

		Ok(())
	}
}
