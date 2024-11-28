use std::io;

use bytesio::bit_reader::BitReader;

use crate::config::AV1CodecConfigurationRecord;
use crate::seq::{ColorConfig, OperatingPoint, SequenceHeaderObu};
use crate::{ObuHeader, ObuType};

#[test]
fn test_config_demux() {
	let data = b"\x81\r\x0c\0\n\x0f\0\0\0j\xef\xbf\xe1\xbc\x02\x19\x90\x10\x10\x10@".to_vec();

	let config = AV1CodecConfigurationRecord::demux(&mut io::Cursor::new(data.into())).unwrap();

	assert!(config.marker);
	assert_eq!(config.version, 1);
	assert_eq!(config.seq_profile, 0);
	assert_eq!(config.seq_level_idx_0, 13);
	assert!(!config.seq_tier_0);
	assert!(!config.high_bitdepth);
	assert!(!config.twelve_bit);
	assert!(!config.monochrome);
	assert!(config.chroma_subsampling_x);
	assert!(config.chroma_subsampling_y);
	assert_eq!(config.initial_presentation_delay_minus_one, None);

	let (header, data) = ObuHeader::parse(&mut BitReader::from(config.config_obu)).unwrap();

	assert_eq!(header.obu_type, ObuType::SequenceHeader);

	let obu = SequenceHeaderObu::parse(header, data).unwrap();

	assert_eq!(
		obu,
		SequenceHeaderObu {
			header: ObuHeader {
				obu_type: ObuType::SequenceHeader,
				extension_flag: false,
				has_size_field: true,
				extension_header: None,
			},
			seq_profile: 0,
			still_picture: false,
			reduced_still_picture_header: false,
			timing_info: None,
			decoder_model_info: None,
			operating_points: vec![OperatingPoint {
				idc: 0,
				seq_level_idx: 13,
				seq_tier: false,
				operating_parameters_info: None,
				initial_display_delay: None,
			}],
			max_frame_width: 3840,
			max_frame_height: 2160,
			frame_ids: None,
			use_128x128_superblock: false,
			enable_filter_intra: false,
			enable_intra_edge_filter: false,
			enable_interintra_compound: false,
			enable_masked_compound: false,
			enable_warped_motion: false,
			enable_dual_filter: false,
			enable_order_hint: true,
			enable_jnt_comp: false,
			enable_ref_frame_mvs: false,
			seq_force_screen_content_tools: 0,
			seq_force_integer_mv: 2,
			order_hint_bits: 7,
			enable_superres: false,
			enable_cdef: true,
			enable_restoration: true,
			color_config: ColorConfig {
				bit_depth: 8,
				mono_chrome: false,
				num_planes: 3,
				color_primaries: 1,
				transfer_characteristics: 1,
				matrix_coefficients: 1,
				full_color_range: false,
				subsampling_x: true,
				subsampling_y: true,
				chroma_sample_position: 0,
				separate_uv_delta_q: false,
			},
			film_grain_params_present: false,
		}
	)
}

#[test]
fn test_config_mux() {
	let data = b"\x81\r\x0c\0\n\x0f\0\0\0j\xef\xbf\xe1\xbc\x02\x19\x90\x10\x10\x10@".to_vec();

	let config = AV1CodecConfigurationRecord::demux(&mut io::Cursor::new(data.clone().into())).unwrap();

	assert_eq!(data.len() as u64, config.size());

	let mut buf = Vec::new();
	config.mux(&mut buf).unwrap();

	assert_eq!(buf, data);
}
