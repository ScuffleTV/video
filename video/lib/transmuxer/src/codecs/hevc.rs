use bytes::Bytes;
use flv::FrameType;
use h265::{HEVCDecoderConfigurationRecord, Sps};
use mp4::types::colr::{ColorType, Colr};
use mp4::types::hev1::Hev1;
use mp4::types::hvcc::HvcC;
use mp4::types::stsd::{SampleEntry, VisualSampleEntry};
use mp4::types::trun::{TrunSample, TrunSampleFlag};
use mp4::DynBox;

use crate::TransmuxError;

pub fn stsd_entry(config: HEVCDecoderConfigurationRecord) -> Result<(DynBox, Sps), TransmuxError> {
	let Some(sps) = config
		.arrays
		.iter()
		.find(|a| a.nal_unit_type == h265::NaluType::Sps)
		.and_then(|v| v.nalus.first())
	else {
		return Err(TransmuxError::InvalidHEVCDecoderConfigurationRecord);
	};

	let sps = h265::Sps::parse(sps.clone())?;

	let colr = sps.color_config.as_ref().map(|color_config| {
		Colr::new(ColorType::Nclx {
			color_primaries: color_config.color_primaries as u16,
			matrix_coefficients: color_config.matrix_coefficients as u16,
			transfer_characteristics: color_config.transfer_characteristics as u16,
			full_range_flag: color_config.full_range,
		})
	});

	Ok((
		Hev1::new(
			SampleEntry::new(VisualSampleEntry::new(sps.width as u16, sps.height as u16, colr)),
			HvcC::new(config),
			None,
		)
		.into(),
		sps,
	))
}

pub fn trun_sample(
	frame_type: FrameType,
	composition_time: i32,
	duration: u32,
	data: &Bytes,
) -> Result<TrunSample, TransmuxError> {
	Ok(TrunSample {
		composition_time_offset: Some(composition_time as i64),
		duration: Some(duration),
		flags: Some(TrunSampleFlag {
			reserved: 0,
			is_leading: 0,
			sample_degradation_priority: 0,
			sample_depends_on: if frame_type == FrameType::Keyframe { 2 } else { 1 },
			sample_has_redundancy: 0,
			sample_is_depended_on: 0,
			sample_is_non_sync_sample: frame_type != FrameType::Keyframe,
			sample_padding_value: 0,
		}),
		size: Some(data.len() as u32),
	})
}
