use bytes::Bytes;

use crate::config::SampleFrequencyIndex;
use crate::{AudioObjectType, AudioSpecificConfig};

#[test]
fn test_aac_config_parse() {
	let data = vec![
		0x12, 0x10, 0x56, 0xe5, 0x00, 0x2d, 0x96, 0x01, 0x80, 0x80, 0x05, 0x00, 0x00, 0x00, 0x00,
	];

	let config = AudioSpecificConfig::parse(Bytes::from(data)).unwrap();
	assert_eq!(config.audio_object_type, AudioObjectType::AacLowComplexity);
	assert_eq!(config.sampling_frequency, 44100);
	assert_eq!(config.channel_configuration, 2);
}

#[test]
fn test_idx_to_freq() {
	assert_eq!(0, SampleFrequencyIndex::FreqEscape.to_freq());
	assert_eq!(0, SampleFrequencyIndex::FreqReserved2.to_freq());
	assert_eq!(0, SampleFrequencyIndex::FreqReserved.to_freq());
	assert_eq!(7350, SampleFrequencyIndex::Freq7350.to_freq());
	assert_eq!(8000, SampleFrequencyIndex::Freq8000.to_freq());
	assert_eq!(11025, SampleFrequencyIndex::Freq11025.to_freq());
	assert_eq!(12000, SampleFrequencyIndex::Freq12000.to_freq());
	assert_eq!(16000, SampleFrequencyIndex::Freq16000.to_freq());
	assert_eq!(22050, SampleFrequencyIndex::Freq22050.to_freq());
	assert_eq!(24000, SampleFrequencyIndex::Freq24000.to_freq());
	assert_eq!(32000, SampleFrequencyIndex::Freq32000.to_freq());
	assert_eq!(44100, SampleFrequencyIndex::Freq44100.to_freq());
	assert_eq!(48000, SampleFrequencyIndex::Freq48000.to_freq());
	assert_eq!(64000, SampleFrequencyIndex::Freq64000.to_freq());
	assert_eq!(88200, SampleFrequencyIndex::Freq88200.to_freq());
	assert_eq!(96000, SampleFrequencyIndex::Freq96000.to_freq());
}
