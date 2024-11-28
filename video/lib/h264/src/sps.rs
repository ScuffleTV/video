use std::io;

use byteorder::{BigEndian, ReadBytesExt};
use bytes::Bytes;
use bytesio::bit_reader::BitReader;
use exp_golomb::{read_exp_golomb, read_signed_exp_golomb};

#[derive(Debug, Clone, PartialEq)]
/// Sequence parameter set
/// ISO/IEC-14496-10-2022 - 7.3.2
pub struct Sps {
	pub profile_idc: u8,
	pub level_idc: u8,
	pub ext: Option<SpsExtended>,
	pub width: u64,
	pub height: u64,
	pub frame_rate: f64,
	pub color_config: Option<ColorConfig>,
}

#[derive(Debug, Clone, PartialEq)]
/// Color config for SPS
pub struct ColorConfig {
	pub full_range: bool,
	pub color_primaries: u8,
	pub transfer_characteristics: u8,
	pub matrix_coefficients: u8,
}

impl Sps {
	pub fn parse(data: Bytes) -> io::Result<Self> {
		let mut vec = Vec::with_capacity(data.len());

		// We need to remove the emulation prevention byte
		// This is BARELY documented in the spec, but it's there.
		// ISO/IEC-14496-10-2022 - 3.1.48
		let mut i = 0;
		while i < data.len() - 3 {
			if data[i] == 0x00 && data[i + 1] == 0x00 && data[i + 2] == 0x03 {
				vec.push(0x00);
				vec.push(0x00);
				i += 3;
			} else {
				vec.push(data[i]);
				i += 1;
			}
		}

		let mut bit_reader = BitReader::from(vec);

		let forbidden_zero_bit = bit_reader.read_bit()?;
		if forbidden_zero_bit {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "Forbidden zero bit is set"));
		}

		bit_reader.seek_bits(2)?; // nal_ref_idc

		let nal_unit_type = bit_reader.read_bits(5)?;
		if nal_unit_type != 7 {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "NAL unit type is not SPS"));
		}

		let profile_idc = bit_reader.read_u8()?;
		bit_reader.seek_bits(
			1 // constraint_set0_flag
            + 1 // constraint_set1_flag
            + 1 // constraint_set2_flag
            + 1 // constraint_set3_flag
            + 4, // reserved_zero_4bits
		)?;

		let level_idc = bit_reader.read_u8()?;
		read_exp_golomb(&mut bit_reader)?; // seq_parameter_set_id

		let sps_ext = match profile_idc {
			100 | 110 | 122 | 244 | 44 | 83 | 86 | 118 | 128 | 138 | 139 | 134 | 135 => {
				Some(SpsExtended::parse(&mut bit_reader)?)
			}
			_ => None,
		};

		read_exp_golomb(&mut bit_reader)?; // log2_max_frame_num_minus4
		let pic_order_cnt_type = read_exp_golomb(&mut bit_reader)?;
		if pic_order_cnt_type == 0 {
			read_exp_golomb(&mut bit_reader)?; // log2_max_pic_order_cnt_lsb_minus4
		} else if pic_order_cnt_type == 1 {
			bit_reader.seek_bits(1)?; // delta_pic_order_always_zero_flag
			read_signed_exp_golomb(&mut bit_reader)?; // offset_for_non_ref_pic
			read_signed_exp_golomb(&mut bit_reader)?; // offset_for_top_to_bottom_field
			let num_ref_frames_in_pic_order_cnt_cycle = read_exp_golomb(&mut bit_reader)?;
			for _ in 0..num_ref_frames_in_pic_order_cnt_cycle {
				read_signed_exp_golomb(&mut bit_reader)?; // offset_for_ref_frame
			}
		}

		read_exp_golomb(&mut bit_reader)?; // max_num_ref_frames
		bit_reader.read_bit()?; // gaps_in_frame_num_value_allowed_flag
		let pic_width_in_mbs_minus1 = read_exp_golomb(&mut bit_reader)?; // pic_width_in_mbs_minus1
		let pic_height_in_map_units_minus1 = read_exp_golomb(&mut bit_reader)?; // pic_height_in_map_units_minus1
		let frame_mbs_only_flag = bit_reader.read_bit()?;
		if !frame_mbs_only_flag {
			bit_reader.seek_bits(1)?; // mb_adaptive_frame_field_flag
		}

		bit_reader.seek_bits(1)?; // direct_8x8_inference_flag

		let mut frame_crop_left_offset = 0;
		let mut frame_crop_right_offset = 0;
		let mut frame_crop_top_offset = 0;
		let mut frame_crop_bottom_offset = 0;

		if bit_reader.read_bit()? {
			// frame_cropping_flag
			frame_crop_left_offset = read_exp_golomb(&mut bit_reader)?; // frame_crop_left_offset
			frame_crop_right_offset = read_exp_golomb(&mut bit_reader)?; // frame_crop_right_offset
			frame_crop_top_offset = read_exp_golomb(&mut bit_reader)?; // frame_crop_top_offset
			frame_crop_bottom_offset = read_exp_golomb(&mut bit_reader)?; // frame_crop_bottom_offset
		}

		let width = ((pic_width_in_mbs_minus1 + 1) * 16) - frame_crop_right_offset * 2 - frame_crop_left_offset * 2;
		let height = ((2 - frame_mbs_only_flag as u64) * (pic_height_in_map_units_minus1 + 1) * 16)
			- frame_crop_bottom_offset * 2
			- frame_crop_top_offset * 2;

		let mut frame_rate = 0.0;

		let vui_parameters_present_flag = bit_reader.read_bit()?;

		let mut color_config = None;

		if vui_parameters_present_flag {
			// We do want to read the VUI parameters to get the frame rate.

			// aspect_ratio_info_present_flag
			if bit_reader.read_bit()? {
				let aspect_ratio_idc = bit_reader.read_u8()?;
				if aspect_ratio_idc == 255 {
					bit_reader.seek_bits(16)?; // sar_width
					bit_reader.seek_bits(16)?; // sar_height
				}
			}

			// overscan_info_present_flag
			if bit_reader.read_bit()? {
				bit_reader.seek_bits(1)?; // overscan_appropriate_flag
			}

			// video_signal_type_present_flag
			if bit_reader.read_bit()? {
				bit_reader.seek_bits(3)?; // video_format
				let full_range = bit_reader.read_bit()?; // video_full_range_flag

				let color_primaries;
				let transfer_characteristics;
				let matrix_coefficients;

				if bit_reader.read_bit()? {
					// colour_description_present_flag
					color_primaries = bit_reader.read_u8()?; // colour_primaries
					transfer_characteristics = bit_reader.read_u8()?; // transfer_characteristics
					matrix_coefficients = bit_reader.read_u8()?; // matrix_coefficients
				} else {
					color_primaries = 2; // UNSPECIFIED
					transfer_characteristics = 2; // UNSPECIFIED
					matrix_coefficients = 2; // UNSPECIFIED
				}

				color_config = Some(ColorConfig {
					full_range,
					color_primaries,
					transfer_characteristics,
					matrix_coefficients,
				});
			}

			// chroma_loc_info_present_flag
			if bit_reader.read_bit()? {
				read_exp_golomb(&mut bit_reader)?; // chroma_sample_loc_type_top_field
				read_exp_golomb(&mut bit_reader)?; // chroma_sample_loc_type_bottom_field
			}

			// timing_info_present_flag
			if bit_reader.read_bit()? {
				let num_units_in_tick = bit_reader.read_u32::<BigEndian>()?;
				let time_scale = bit_reader.read_u32::<BigEndian>()?;
				frame_rate = time_scale as f64 / (2.0 * num_units_in_tick as f64);
			}
		}

		Ok(Sps {
			profile_idc,
			level_idc,
			ext: sps_ext,
			width,
			height,
			frame_rate,
			color_config,
		})
	}
}

#[derive(Debug, Clone, PartialEq)]
/// Sequence parameter set extension.
/// ISO/IEC-14496-10-2022 - 7.3.2
pub struct SpsExtended {
	pub chroma_format_idc: u64,       // ue(v)
	pub bit_depth_luma_minus8: u64,   // ue(v)
	pub bit_depth_chroma_minus8: u64, // ue(v)
}

impl SpsExtended {
	pub fn parse(reader: &mut BitReader) -> io::Result<Self> {
		let chroma_format_idc = read_exp_golomb(reader)?;
		if chroma_format_idc == 3 {
			reader.seek_bits(1)?;
		}

		let bit_depth_luma_minus8 = read_exp_golomb(reader)?;
		let bit_depth_chroma_minus8 = read_exp_golomb(reader)?;
		reader.seek_bits(1)?; // qpprime_y_zero_transform_bypass_flag

		if reader.read_bit()? {
			// seq_scaling_matrix_present_flag
			// We need to read the scaling matrices here, but we don't need them
			// for decoding, so we just skip them.
			let count = if chroma_format_idc != 3 { 8 } else { 12 };
			for i in 0..count {
				if reader.read_bit()? {
					let size = if i < 6 { 16 } else { 64 };
					let mut next_scale = 8;
					for _ in 0..size {
						let delta_scale = read_signed_exp_golomb(reader)?;
						next_scale = (next_scale + delta_scale + 256) % 256;
						if next_scale == 0 {
							break;
						}
					}
				}
			}
		}

		Ok(SpsExtended {
			chroma_format_idc,
			bit_depth_luma_minus8,
			bit_depth_chroma_minus8,
		})
	}
}
