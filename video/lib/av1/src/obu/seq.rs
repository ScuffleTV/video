use std::io;

use byteorder::{BigEndian, ReadBytesExt};
use bytes::Bytes;
use bytesio::bit_reader::BitReader;

use super::ObuHeader;
use crate::obu::read_uvlc;

#[derive(Debug, Clone, PartialEq)]
/// Sequence Header OBU
/// AV1-Spec-2 - 5.5
pub struct SequenceHeaderObu {
	pub header: ObuHeader,
	pub seq_profile: u8,
	pub still_picture: bool,
	pub reduced_still_picture_header: bool,
	pub timing_info: Option<TimingInfo>,
	pub decoder_model_info: Option<DecoderModelInfo>,
	pub operating_points: Vec<OperatingPoint>,
	pub max_frame_width: u64,
	pub max_frame_height: u64,
	pub frame_ids: Option<FrameIds>,
	pub use_128x128_superblock: bool,
	pub enable_filter_intra: bool,
	pub enable_intra_edge_filter: bool,
	pub enable_interintra_compound: bool,
	pub enable_masked_compound: bool,
	pub enable_warped_motion: bool,
	pub enable_dual_filter: bool,
	pub enable_order_hint: bool,
	pub enable_jnt_comp: bool,
	pub enable_ref_frame_mvs: bool,
	pub seq_force_screen_content_tools: u8,
	pub seq_force_integer_mv: u8,
	pub order_hint_bits: u8,
	pub enable_superres: bool,
	pub enable_cdef: bool,
	pub enable_restoration: bool,
	pub color_config: ColorConfig,
	pub film_grain_params_present: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FrameIds {
	pub delta_frame_id_length: u8,
	pub additional_frame_id_length: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OperatingPoint {
	pub idc: u16,
	pub seq_level_idx: u8,
	pub seq_tier: bool,
	pub operating_parameters_info: Option<OperatingParametersInfo>,
	pub initial_display_delay: Option<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TimingInfo {
	pub num_units_in_display_tick: u32,
	pub time_scale: u32,
	pub num_ticks_per_picture: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DecoderModelInfo {
	pub buffer_delay_length: u8,
	pub num_units_in_decoding_tick: u32,
	pub buffer_removal_time_length: u8,
	pub frame_presentation_time_length: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OperatingParametersInfo {
	pub decoder_buffer_delay: u64,
	pub encoder_buffer_delay: u64,
	pub low_delay_mode_flag: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColorConfig {
	pub bit_depth: i32,
	pub mono_chrome: bool,
	pub num_planes: u8,
	pub color_primaries: u8,
	pub transfer_characteristics: u8,
	pub matrix_coefficients: u8,
	pub full_color_range: bool,
	pub subsampling_x: bool,
	pub subsampling_y: bool,
	pub chroma_sample_position: u8,
	pub separate_uv_delta_q: bool,
}

impl SequenceHeaderObu {
	pub fn header(&self) -> &ObuHeader {
		&self.header
	}

	pub fn parse(header: ObuHeader, data: Bytes) -> io::Result<Self> {
		let mut bit_reader = BitReader::from(data);

		let seq_profile = bit_reader.read_bits(3)? as u8;
		let still_picture = bit_reader.read_bit()?;
		let reduced_still_picture_header = bit_reader.read_bit()?;

		let mut timing_info = None;
		let mut decoder_model_info = None;
		let mut operating_points = Vec::new();

		if reduced_still_picture_header {
			operating_points.push(OperatingPoint {
				idc: 0,
				seq_level_idx: bit_reader.read_bits(5)? as u8,
				seq_tier: false,
				operating_parameters_info: None,
				initial_display_delay: None,
			});
		} else {
			let timing_info_present_flag = bit_reader.read_bit()?;
			if timing_info_present_flag {
				let num_units_in_display_tick = bit_reader.read_u32::<BigEndian>()?;
				let time_scale = bit_reader.read_u32::<BigEndian>()?;
				let num_ticks_per_picture = if bit_reader.read_bit()? {
					Some(read_uvlc(&mut bit_reader)? + 1)
				} else {
					None
				};
				timing_info = Some(TimingInfo {
					num_units_in_display_tick,
					time_scale,
					num_ticks_per_picture,
				});

				let decoder_model_info_present_flag = bit_reader.read_bit()?;
				if decoder_model_info_present_flag {
					let buffer_delay_length = bit_reader.read_bits(5)? as u8 + 1;
					let num_units_in_decoding_tick = bit_reader.read_u32::<BigEndian>()?;
					let buffer_removal_time_length = bit_reader.read_bits(5)? as u8 + 1;
					let frame_presentation_time_length = bit_reader.read_bits(5)? as u8 + 1;
					decoder_model_info = Some(DecoderModelInfo {
						buffer_delay_length,
						num_units_in_decoding_tick,
						buffer_removal_time_length,
						frame_presentation_time_length,
					});
				}
			}

			let initial_display_delay_present_flag = bit_reader.read_bit()?;
			let operating_points_cnt_minus_1 = bit_reader.read_bits(5)? as u8;
			for _ in 0..=operating_points_cnt_minus_1 {
				let idc = bit_reader.read_bits(12)? as u16;
				let seq_level_idx = bit_reader.read_bits(5)? as u8;
				let seq_tier = if seq_level_idx > 7 { bit_reader.read_bit()? } else { false };
				let decoder_model_present_for_this_op = if decoder_model_info.is_some() {
					bit_reader.read_bit()?
				} else {
					false
				};

				let operating_parameters_info = if decoder_model_present_for_this_op {
					let decoder_buffer_delay =
						bit_reader.read_bits(decoder_model_info.as_ref().unwrap().buffer_delay_length)?;
					let encoder_buffer_delay =
						bit_reader.read_bits(decoder_model_info.as_ref().unwrap().buffer_delay_length)?;
					let low_delay_mode_flag = bit_reader.read_bit()?;
					Some(OperatingParametersInfo {
						decoder_buffer_delay,
						encoder_buffer_delay,
						low_delay_mode_flag,
					})
				} else {
					None
				};

				let initial_display_delay = if initial_display_delay_present_flag {
					if bit_reader.read_bit()? {
						// initial_display_delay_present_for_this_op
						Some(bit_reader.read_bits(4)? as u8 + 1) // initial_display_delay_minus_1
					} else {
						None
					}
				} else {
					None
				};

				operating_points.push(OperatingPoint {
					idc,
					seq_level_idx,
					seq_tier,
					operating_parameters_info,
					initial_display_delay,
				});
			}
		}

		if operating_points.is_empty() {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"seq_obu parse error: no operating points",
			));
		}

		let frame_width_bits = bit_reader.read_bits(4)? as u8 + 1;
		let frame_height_bits = bit_reader.read_bits(4)? as u8 + 1;

		let max_frame_width = bit_reader.read_bits(frame_width_bits)? + 1;
		let max_frame_height = bit_reader.read_bits(frame_height_bits)? + 1;

		let frame_id_numbers_present_flag = if reduced_still_picture_header {
			false
		} else {
			bit_reader.read_bit()?
		};
		let frame_ids = if frame_id_numbers_present_flag {
			let delta_frame_id_length = bit_reader.read_bits(4)? as u8 + 2;
			let additional_frame_id_length = bit_reader.read_bits(3)? as u8 + 1;
			Some(FrameIds {
				delta_frame_id_length,
				additional_frame_id_length,
			})
		} else {
			None
		};

		let use_128x128_superblock = bit_reader.read_bit()?;
		let enable_filter_intra = bit_reader.read_bit()?;
		let enable_intra_edge_filter = bit_reader.read_bit()?;

		let enable_interintra_compound;
		let enable_masked_compound;
		let enable_warped_motion;
		let enable_dual_filter;
		let enable_order_hint;
		let enable_jnt_comp;
		let enable_ref_frame_mvs;
		let order_hint_bits;
		let seq_force_integer_mv;

		let seq_force_screen_content_tools;

		if !reduced_still_picture_header {
			enable_interintra_compound = bit_reader.read_bit()?;
			enable_masked_compound = bit_reader.read_bit()?;
			enable_warped_motion = bit_reader.read_bit()?;
			enable_dual_filter = bit_reader.read_bit()?;
			enable_order_hint = bit_reader.read_bit()?;
			if enable_order_hint {
				enable_jnt_comp = bit_reader.read_bit()?;
				enable_ref_frame_mvs = bit_reader.read_bit()?;
			} else {
				enable_jnt_comp = false;
				enable_ref_frame_mvs = false;
			}
			if bit_reader.read_bit()? {
				// seq_choose_screen_content_tools
				seq_force_screen_content_tools = 2; // SELECT_SCREEN_CONTENT_TOOLS
			} else {
				seq_force_screen_content_tools = bit_reader.read_bits(1)? as u8;
			}

			// If seq_force_screen_content_tools is 0, then seq_force_integer_mv must be 2.
			// Or if the next bit is 0, then seq_force_integer_mv must be 2.
			if seq_force_screen_content_tools == 0 || bit_reader.read_bit()? {
				seq_force_integer_mv = 2; // SELECT_INTEGER_MV
			} else {
				seq_force_integer_mv = bit_reader.read_bits(1)? as u8;
			}

			if enable_order_hint {
				order_hint_bits = bit_reader.read_bits(3)? as u8 + 1;
			} else {
				order_hint_bits = 0;
			}
		} else {
			enable_interintra_compound = false;
			enable_masked_compound = false;
			enable_warped_motion = false;
			enable_dual_filter = false;
			enable_order_hint = false;
			enable_jnt_comp = false;
			enable_ref_frame_mvs = false;
			seq_force_screen_content_tools = 2; // SELECT_SCREEN_CONTENT_TOOLS
			seq_force_integer_mv = 2; // SELECT_INTEGER_MV
			order_hint_bits = 0;
		}

		let enable_superres = bit_reader.read_bit()?;
		let enable_cdef = bit_reader.read_bit()?;
		let enable_restoration = bit_reader.read_bit()?;

		let high_bitdepth = bit_reader.read_bit()?;
		let bit_depth = if seq_profile == 2 && high_bitdepth {
			if bit_reader.read_bit()? { 12 } else { 10 }
		} else if high_bitdepth {
			10
		} else {
			8
		};

		let mono_chrome = if seq_profile == 1 { false } else { bit_reader.read_bit()? };

		let color_primaries;
		let transfer_characteristics;
		let matrix_coefficients;

		let color_description_present_flag = bit_reader.read_bit()?;
		if color_description_present_flag {
			color_primaries = bit_reader.read_bits(8)? as u8;
			transfer_characteristics = bit_reader.read_bits(8)? as u8;
			matrix_coefficients = bit_reader.read_bits(8)? as u8;
		} else {
			color_primaries = 2; // CP_UNSPECIFIED
			transfer_characteristics = 2; // TC_UNSPECIFIED
			matrix_coefficients = 2; // MC_UNSPECIFIED
		}

		let num_planes = if mono_chrome { 1 } else { 3 };

		let color_config;

		if mono_chrome {
			let color_range = bit_reader.read_bit()?;
			let subsampling_x = true;
			let subsampling_y = true;
			color_config = ColorConfig {
				bit_depth,
				color_primaries,
				transfer_characteristics,
				matrix_coefficients,
				full_color_range: color_range,
				subsampling_x,
				subsampling_y,
				mono_chrome,
				separate_uv_delta_q: false,
				chroma_sample_position: 0, // CSP_UNKNOWN
				num_planes,
			}
		} else {
			let color_range;
			let subsampling_x;
			let subsampling_y;

			// color_primarties == CP_BT_709 && transfer_characteristics == TC_SRGB &&
			// matrix_coefficients == MC_IDENTITY
			if color_primaries == 1 && transfer_characteristics == 13 && matrix_coefficients == 0 {
				color_range = true;
				subsampling_x = false;
				subsampling_y = false;
			} else {
				color_range = bit_reader.read_bit()?;
				if seq_profile == 0 {
					subsampling_x = true;
					subsampling_y = true;
				} else if seq_profile == 1 {
					subsampling_x = false;
					subsampling_y = false;
				} else if bit_depth == 12 {
					subsampling_x = bit_reader.read_bit()?;
					if subsampling_x {
						subsampling_y = bit_reader.read_bit()?;
					} else {
						subsampling_y = false;
					}
				} else {
					subsampling_x = true;
					subsampling_y = false;
				}
			}

			let chroma_sample_position = if subsampling_x && subsampling_y {
				bit_reader.read_bits(2)? as u8
			} else {
				0 // CSP_UNKNOWN
			};

			let separate_uv_delta_q = bit_reader.read_bit()?;
			color_config = ColorConfig {
				bit_depth,
				mono_chrome,
				color_primaries,
				transfer_characteristics,
				matrix_coefficients,
				full_color_range: color_range,
				subsampling_x,
				subsampling_y,
				chroma_sample_position,
				separate_uv_delta_q,
				num_planes,
			};
		}

		let film_grain_params_present = bit_reader.read_bit()?;

		Ok(Self {
			header,
			seq_profile,
			still_picture,
			reduced_still_picture_header,
			operating_points,
			decoder_model_info,
			max_frame_width,
			max_frame_height,
			frame_ids,
			use_128x128_superblock,
			enable_filter_intra,
			enable_intra_edge_filter,
			enable_interintra_compound,
			enable_masked_compound,
			enable_warped_motion,
			enable_dual_filter,
			enable_order_hint,
			enable_jnt_comp,
			enable_ref_frame_mvs,
			seq_force_screen_content_tools,
			seq_force_integer_mv,
			order_hint_bits,
			enable_superres,
			enable_cdef,
			enable_restoration,
			timing_info,
			color_config,
			film_grain_params_present,
		})
	}
}
