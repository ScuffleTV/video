use std::io;

use bytes::Bytes;

use crate::sps::{ColorConfig, Sps};
use crate::{HEVCDecoderConfigurationRecord, NaluType};

#[test]
fn test_sps_parse() {
	let data = b"B\x01\x01\x01@\0\0\x03\0\x90\0\0\x03\0\0\x03\0\x99\xa0\x01@ \x05\xa1e\x95R\x90\x84d_\xf8\xc0Z\x80\x80\x80\x82\0\0\x03\0\x02\0\0\x03\x01 \xc0\x0b\xbc\xa2\0\x02bX\0\x011-\x08".to_vec();

	let sps = Sps::parse(Bytes::from(data.to_vec())).unwrap();
	assert_eq!(
		sps,
		Sps {
			color_config: Some(ColorConfig {
				full_range: false,
				color_primaries: 1,
				matrix_coefficients: 1,
				transfer_characteristics: 1,
			}),
			frame_rate: 144.0,
			width: 2560,
			height: 1440,
		}
	);
}

#[test]
fn test_config_demux() {
	// h265 config
	let data = Bytes::from(b"\x01\x01@\0\0\0\x90\0\0\0\0\0\x99\xf0\0\xfc\xfd\xf8\xf8\0\0\x0f\x03 \0\x01\0\x18@\x01\x0c\x01\xff\xff\x01@\0\0\x03\0\x90\0\0\x03\0\0\x03\0\x99\x95@\x90!\0\x01\0=B\x01\x01\x01@\0\0\x03\0\x90\0\0\x03\0\0\x03\0\x99\xa0\x01@ \x05\xa1e\x95R\x90\x84d_\xf8\xc0Z\x80\x80\x80\x82\0\0\x03\0\x02\0\0\x03\x01 \xc0\x0b\xbc\xa2\0\x02bX\0\x011-\x08\"\0\x01\0\x07D\x01\xc0\x93|\x0c\xc9".to_vec());

	let config = HEVCDecoderConfigurationRecord::demux(&mut io::Cursor::new(data)).unwrap();

	assert_eq!(config.configuration_version, 1);
	assert_eq!(config.general_profile_space, 0);
	assert!(!config.general_tier_flag);
	assert_eq!(config.general_profile_idc, 1);
	assert_eq!(config.general_profile_compatibility_flags, 64);
	assert_eq!(config.general_constraint_indicator_flags, 144);
	assert_eq!(config.general_level_idc, 153);
	assert_eq!(config.min_spatial_segmentation_idc, 0);
	assert_eq!(config.parallelism_type, 0);
	assert_eq!(config.chroma_format_idc, 1);
	assert_eq!(config.bit_depth_luma_minus8, 0);
	assert_eq!(config.bit_depth_chroma_minus8, 0);
	assert_eq!(config.avg_frame_rate, 0);
	assert_eq!(config.constant_frame_rate, 0);
	assert_eq!(config.num_temporal_layers, 1);
	assert!(config.temporal_id_nested);
	assert_eq!(config.length_size_minus_one, 3);
	assert_eq!(config.arrays.len(), 3);

	let vps = &config.arrays[0];
	assert!(!vps.array_completeness);
	assert_eq!(vps.nal_unit_type, NaluType::Vps);
	assert_eq!(vps.nalus.len(), 1);

	let sps = &config.arrays[1];
	assert!(!sps.array_completeness);
	assert_eq!(sps.nal_unit_type, NaluType::Sps);
	assert_eq!(sps.nalus.len(), 1);
	let sps = Sps::parse(sps.nalus[0].clone()).unwrap();
	assert_eq!(
		sps,
		Sps {
			color_config: Some(ColorConfig {
				full_range: false,
				color_primaries: 1,
				matrix_coefficients: 1,
				transfer_characteristics: 1,
			}),
			frame_rate: 144.0,
			width: 2560,
			height: 1440,
		}
	);

	let pps = &config.arrays[2];
	assert!(!pps.array_completeness);
	assert_eq!(pps.nal_unit_type, NaluType::Pps);
	assert_eq!(pps.nalus.len(), 1);
}

#[test]
fn test_config_mux() {
	let data = Bytes::from(b"\x01\x01@\0\0\0\x90\0\0\0\0\0\x99\xf0\0\xfc\xfd\xf8\xf8\0\0\x0f\x03 \0\x01\0\x18@\x01\x0c\x01\xff\xff\x01@\0\0\x03\0\x90\0\0\x03\0\0\x03\0\x99\x95@\x90!\0\x01\0=B\x01\x01\x01@\0\0\x03\0\x90\0\0\x03\0\0\x03\0\x99\xa0\x01@ \x05\xa1e\x95R\x90\x84d_\xf8\xc0Z\x80\x80\x80\x82\0\0\x03\0\x02\0\0\x03\x01 \xc0\x0b\xbc\xa2\0\x02bX\0\x011-\x08\"\0\x01\0\x07D\x01\xc0\x93|\x0c\xc9".to_vec());

	let config = HEVCDecoderConfigurationRecord::demux(&mut io::Cursor::new(data.clone())).unwrap();

	assert_eq!(config.size(), data.len() as u64);

	let mut buf = Vec::new();
	config.mux(&mut buf).unwrap();

	assert_eq!(buf, data.to_vec());
}
