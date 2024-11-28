use bytes::Bytes;
use flv::FrameType;
use h264::{AVCDecoderConfigurationRecord, Sps};
use mp4::types::avc1::Avc1;
use mp4::types::avcc::AvcC;
use mp4::types::colr::{ColorType, Colr};
use mp4::types::stsd::{SampleEntry, VisualSampleEntry};
use mp4::types::trun::{TrunSample, TrunSampleFlag};
use mp4::DynBox;

use crate::TransmuxError;

pub fn stsd_entry(config: AVCDecoderConfigurationRecord) -> Result<(DynBox, Sps), TransmuxError> {
	if config.sps.is_empty() {
		return Err(TransmuxError::InvalidAVCDecoderConfigurationRecord);
	}

	let sps = h264::Sps::parse(config.sps[0].clone())?;

	let colr = sps.color_config.as_ref().map(|color_config| {
		Colr::new(ColorType::Nclx {
			color_primaries: color_config.color_primaries as u16,
			matrix_coefficients: color_config.matrix_coefficients as u16,
			transfer_characteristics: color_config.transfer_characteristics as u16,
			full_range_flag: color_config.full_range,
		})
	});

	Ok((
		Avc1::new(
			SampleEntry::new(VisualSampleEntry::new(sps.width as u16, sps.height as u16, colr)),
			AvcC::new(config),
			None,
		)
		.into(),
		sps,
	))
}

pub fn trun_sample(
	frame_type: FrameType,
	composition_time: u32,
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
