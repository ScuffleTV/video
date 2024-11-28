use av1::seq::SequenceHeaderObu;
use av1::{AV1CodecConfigurationRecord, ObuHeader, ObuType};
use bytes::Bytes;
use bytesio::bit_reader::BitReader;
use flv::FrameType;
use mp4::types::av01::Av01;
use mp4::types::av1c::Av1C;
use mp4::types::colr::{ColorType, Colr};
use mp4::types::stsd::{SampleEntry, VisualSampleEntry};
use mp4::types::trun::{TrunSample, TrunSampleFlag};
use mp4::DynBox;

use crate::TransmuxError;

pub fn stsd_entry(config: AV1CodecConfigurationRecord) -> Result<(DynBox, SequenceHeaderObu), TransmuxError> {
	let (header, data) = ObuHeader::parse(&mut BitReader::from(config.config_obu.clone()))?;

	if header.obu_type != ObuType::SequenceHeader {
		return Err(TransmuxError::InvalidAv1DecoderConfigurationRecord);
	}

	let seq_obu = SequenceHeaderObu::parse(header, data)?;

	// Unfortunate there does not seem to be a way to get the
	// frame rate from the sequence header unless the timing_info is present
	// Which it almost never is.
	// So for AV1 we rely on the framerate being set in the scriptdata tag

	Ok((
		Av01::new(
			SampleEntry::new(VisualSampleEntry::new(
				seq_obu.max_frame_width as u16,
				seq_obu.max_frame_height as u16,
				Some(Colr::new(ColorType::Nclx {
					color_primaries: seq_obu.color_config.color_primaries as u16,
					matrix_coefficients: seq_obu.color_config.matrix_coefficients as u16,
					transfer_characteristics: seq_obu.color_config.transfer_characteristics as u16,
					full_range_flag: seq_obu.color_config.full_color_range,
				})),
			)),
			Av1C::new(config),
			None,
		)
		.into(),
		seq_obu,
	))
}

pub fn trun_sample(frame_type: FrameType, duration: u32, data: &Bytes) -> Result<TrunSample, TransmuxError> {
	Ok(TrunSample {
		composition_time_offset: None,
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
