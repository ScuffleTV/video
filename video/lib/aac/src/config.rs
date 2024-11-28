use std::io;

use bytes::Bytes;
use bytesio::bit_reader::BitReader;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

#[derive(Debug, Clone, PartialEq)]
/// Audio Specific Config
/// ISO/IEC 14496-3:2019(E) - 1.6
pub struct AudioSpecificConfig {
	pub audio_object_type: AudioObjectType,
	pub sampling_frequency: u32,
	pub channel_configuration: u8,
	pub data: Bytes,
}

#[derive(Debug, Clone, PartialEq, Copy, Eq)]
/// SBR Audio Object Type
/// ISO/IEC 14496-3:2019(E) - 1.5.1.2.6
pub enum AudioObjectType {
	AacMain,
	AacLowComplexity,
	Unknown(u16),
}

impl From<u16> for AudioObjectType {
	fn from(value: u16) -> Self {
		match value {
			1 => AudioObjectType::AacMain,
			2 => AudioObjectType::AacLowComplexity,
			_ => AudioObjectType::Unknown(value),
		}
	}
}

impl From<AudioObjectType> for u16 {
	fn from(value: AudioObjectType) -> Self {
		match value {
			AudioObjectType::AacMain => 1,
			AudioObjectType::AacLowComplexity => 2,
			AudioObjectType::Unknown(value) => value,
		}
	}
}

#[derive(FromPrimitive)]
#[repr(u8)]
/// Sampling Frequency Index
/// ISO/IEC 14496-3:2019(E) - 1.6.2.4 (Table 1.22)
pub enum SampleFrequencyIndex {
	Freq96000 = 0x0,
	Freq88200 = 0x1,
	Freq64000 = 0x2,
	Freq48000 = 0x3,
	Freq44100 = 0x4,
	Freq32000 = 0x5,
	Freq24000 = 0x6,
	Freq22050 = 0x7,
	Freq16000 = 0x8,
	Freq12000 = 0x9,
	Freq11025 = 0xA,
	Freq8000 = 0xB,
	Freq7350 = 0xC,
	FreqReserved = 0xD,
	FreqReserved2 = 0xE,
	FreqEscape = 0xF,
}

impl SampleFrequencyIndex {
	pub fn to_freq(&self) -> u32 {
		match self {
			SampleFrequencyIndex::Freq96000 => 96000,
			SampleFrequencyIndex::Freq88200 => 88200,
			SampleFrequencyIndex::Freq64000 => 64000,
			SampleFrequencyIndex::Freq48000 => 48000,
			SampleFrequencyIndex::Freq44100 => 44100,
			SampleFrequencyIndex::Freq32000 => 32000,
			SampleFrequencyIndex::Freq24000 => 24000,
			SampleFrequencyIndex::Freq22050 => 22050,
			SampleFrequencyIndex::Freq16000 => 16000,
			SampleFrequencyIndex::Freq12000 => 12000,
			SampleFrequencyIndex::Freq11025 => 11025,
			SampleFrequencyIndex::Freq8000 => 8000,
			SampleFrequencyIndex::Freq7350 => 7350,
			SampleFrequencyIndex::FreqReserved => 0,
			SampleFrequencyIndex::FreqReserved2 => 0,
			SampleFrequencyIndex::FreqEscape => 0,
		}
	}
}

impl AudioSpecificConfig {
	pub fn parse(data: Bytes) -> io::Result<Self> {
		let mut bitreader = BitReader::from(data);
		let mut audio_object_type = bitreader.read_bits(5)? as u16;
		if audio_object_type == 31 {
			audio_object_type = 32 + bitreader.read_bits(6)? as u16;
		}

		let sampling_frequency_index = SampleFrequencyIndex::from_u8(bitreader.read_bits(4)? as u8)
			.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Invalid sampling frequency index"))?;
		let sampling_frequency = match sampling_frequency_index {
			SampleFrequencyIndex::FreqEscape => bitreader.read_bits(24)? as u32,
			_ => sampling_frequency_index.to_freq(),
		};

		let channel_configuration = bitreader.read_bits(4)? as u8;

		Ok(Self {
			audio_object_type: audio_object_type.into(),
			sampling_frequency,
			channel_configuration,
			data: bitreader.into_inner().into_inner(),
		})
	}
}
