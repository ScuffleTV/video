use std::io;

use bytes::Bytes;

use crate::config::{AVCDecoderConfigurationRecord, AvccExtendedConfig};
use crate::sps::{ColorConfig, Sps, SpsExtended};

#[test]
fn test_parse_sps() {
	let sps = Bytes::from(vec![
		103, 100, 0, 51, 172, 202, 80, 15, 0, 16, 251, 1, 16, 0, 0, 3, 0, 16, 0, 0, 7, 136, 241, 131, 25, 96,
	]);

	let sps = Sps::parse(sps).unwrap();

	assert_eq!(sps.profile_idc, 100);
	assert_eq!(sps.level_idc, 51);
	assert_eq!(
		sps.ext,
		Some(SpsExtended {
			chroma_format_idc: 1,
			bit_depth_luma_minus8: 0,
			bit_depth_chroma_minus8: 0,
		})
	);
	assert_eq!(sps.width, 3840);
	assert_eq!(sps.height, 2160);
	assert_eq!(sps.frame_rate, 60.0);
	assert_eq!(sps.color_config, None);
}

#[test]
fn test_parse_sps2() {
	let sps = Bytes::from(vec![
		0x67, 0x42, 0xc0, 0x1f, 0x8c, 0x8d, 0x40, 0x50, 0x1e, 0x90, 0x0f, 0x08, 0x84, 0x6a,
	]);

	let sps = Sps::parse(sps).unwrap();

	assert_eq!(sps.profile_idc, 66);
	assert_eq!(sps.level_idc, 31);
	assert_eq!(sps.ext, None);
	assert_eq!(sps.width, 640);
	assert_eq!(sps.height, 480);
	assert_eq!(sps.frame_rate, 0.0);
	assert_eq!(sps.color_config, None);
}

#[test]
fn test_parse_sps3() {
	let sps = Bytes::from(vec![
		103, 100, 0, 42, 172, 178, 0, 240, 4, 79, 203, 128, 181, 1, 1, 1, 64, 0, 0, 3, 0, 64, 0, 0, 30, 35, 198, 12, 146,
	]);

	let sps = Sps::parse(sps).unwrap();

	assert_eq!(sps.profile_idc, 100);
	assert_eq!(sps.level_idc, 42);
	assert_eq!(
		sps.ext,
		Some(SpsExtended {
			chroma_format_idc: 1,
			bit_depth_luma_minus8: 0,
			bit_depth_chroma_minus8: 0,
		})
	);
	assert_eq!(sps.width, 1920);
	assert_eq!(sps.height, 1080);
	assert_eq!(sps.frame_rate, 60.0);
	assert_eq!(
		sps.color_config,
		Some(ColorConfig {
			full_range: false,
			matrix_coefficients: 1,
			color_primaries: 1,
			transfer_characteristics: 1,
		})
	);
}

#[test]
fn test_config_demux() {
	let data = Bytes::from(b"\x01d\0\x1f\xff\xe1\0\x1dgd\0\x1f\xac\xd9A\xe0m\xf9\xe6\xa0  (\0\0\x03\0\x08\0\0\x03\x01\xe0x\xc1\x8c\xb0\x01\0\x06h\xeb\xe3\xcb\"\xc0\xfd\xf8\xf8\0".to_vec());

	let config = AVCDecoderConfigurationRecord::demux(&mut io::Cursor::new(data)).unwrap();

	assert_eq!(config.configuration_version, 1);
	assert_eq!(config.profile_indication, 100);
	assert_eq!(config.profile_compatibility, 0);
	assert_eq!(config.level_indication, 31);
	assert_eq!(config.length_size_minus_one, 3);
	assert_eq!(
		config.extended_config,
		Some(AvccExtendedConfig {
			bit_depth_chroma_minus8: 0,
			bit_depth_luma_minus8: 0,
			chroma_format: 1,
			sequence_parameter_set_ext: vec![],
		})
	);

	assert_eq!(config.sps.len(), 1);
	assert_eq!(config.pps.len(), 1);

	let sps = &config.sps[0];
	let sps = Sps::parse(sps.clone()).unwrap();

	assert_eq!(sps.profile_idc, 100);
	assert_eq!(sps.level_idc, 31);
	assert_eq!(
		sps.ext,
		Some(SpsExtended {
			chroma_format_idc: 1,
			bit_depth_luma_minus8: 0,
			bit_depth_chroma_minus8: 0,
		})
	);

	assert_eq!(sps.width, 480);
	assert_eq!(sps.height, 852);
	assert_eq!(sps.frame_rate, 30.0);
	assert_eq!(
		sps.color_config,
		Some(ColorConfig {
			full_range: false,
			matrix_coefficients: 1,
			color_primaries: 1,
			transfer_characteristics: 1,
		})
	)
}

#[test]
fn test_config_mux() {
	let data = Bytes::from(b"\x01d\0\x1f\xff\xe1\0\x1dgd\0\x1f\xac\xd9A\xe0m\xf9\xe6\xa0  (\0\0\x03\0\x08\0\0\x03\x01\xe0x\xc1\x8c\xb0\x01\0\x06h\xeb\xe3\xcb\"\xc0\xfd\xf8\xf8\0".to_vec());

	let config = AVCDecoderConfigurationRecord::demux(&mut io::Cursor::new(data.clone())).unwrap();

	assert_eq!(config.size(), data.len() as u64);

	let mut buf = Vec::new();
	config.mux(&mut buf).unwrap();

	assert_eq!(buf, data.to_vec());
}
