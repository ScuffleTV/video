use aac::AudioSpecificConfig;
use bytes::Bytes;
use flv::{SoundSize, SoundType};
use mp4::types::esds::descriptor::header::DescriptorHeader;
use mp4::types::esds::descriptor::traits::DescriptorType;
use mp4::types::esds::descriptor::types::decoder_config::DecoderConfigDescriptor;
use mp4::types::esds::descriptor::types::decoder_specific_info::DecoderSpecificInfoDescriptor;
use mp4::types::esds::descriptor::types::es::EsDescriptor;
use mp4::types::esds::Esds;
use mp4::types::mp4a::Mp4a;
use mp4::types::stsd::{AudioSampleEntry, SampleEntry};
use mp4::types::trun::{TrunSample, TrunSampleFlag};
use mp4::DynBox;

use crate::TransmuxError;

pub fn stsd_entry(
	sound_size: SoundSize,
	sound_type: SoundType,
	data: Bytes,
) -> Result<(DynBox, AudioSpecificConfig), TransmuxError> {
	let aac_config = aac::AudioSpecificConfig::parse(data)?;

	Ok((
		Mp4a::new(
			SampleEntry::new(AudioSampleEntry::new(
				match sound_type {
					SoundType::Mono => 1,
					SoundType::Stereo => 2,
				},
				match sound_size {
					SoundSize::Bit8 => 8,
					SoundSize::Bit16 => 16,
				},
				aac_config.sampling_frequency,
			)),
			Esds::new(EsDescriptor::new(
				2,
				0,
				Some(0),
				None,
				Some(0),
				Some(DecoderConfigDescriptor::new(
					0x40, // aac
					0x05, // audio stream
					0,    // max bitrate
					0,    // avg bitrate
					Some(DecoderSpecificInfoDescriptor {
						header: DescriptorHeader::new(DecoderSpecificInfoDescriptor::TAG),
						data: aac_config.data.clone(),
					}),
				)),
				None,
			)),
			None,
		)
		.into(),
		aac_config,
	))
}

pub fn trun_sample(data: &Bytes) -> Result<(TrunSample, u32), TransmuxError> {
	Ok((
		TrunSample {
			duration: Some(1024),
			composition_time_offset: None,
			flags: Some(TrunSampleFlag {
				reserved: 0,
				is_leading: 0,
				sample_degradation_priority: 0,
				sample_depends_on: 2,
				sample_has_redundancy: 0,
				sample_is_depended_on: 0,
				sample_is_non_sync_sample: false,
				sample_padding_value: 0,
			}),
			size: Some(data.len() as u32),
		},
		1024,
	))
}
