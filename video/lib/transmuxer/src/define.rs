use av1::AV1CodecConfigurationRecord;
use bytes::Bytes;
use flv::{SoundSize, SoundType};
use h264::AVCDecoderConfigurationRecord;
use h265::HEVCDecoderConfigurationRecord;
use mp4::codec::{AudioCodec, VideoCodec};

pub(crate) enum VideoSequenceHeader {
	Avc(AVCDecoderConfigurationRecord),
	Hevc(HEVCDecoderConfigurationRecord),
	Av1(AV1CodecConfigurationRecord),
}

pub(crate) struct AudioSequenceHeader {
	pub sound_size: SoundSize,
	pub sound_type: SoundType,
	pub data: AudioSequenceHeaderData,
}

pub(crate) enum AudioSequenceHeaderData {
	Aac(Bytes),
}

#[derive(Debug, Clone)]
pub enum TransmuxResult {
	InitSegment {
		video_settings: VideoSettings,
		audio_settings: AudioSettings,
		data: Bytes,
	},
	MediaSegment(MediaSegment),
}

#[derive(Debug, Clone, PartialEq)]
pub struct VideoSettings {
	pub width: u32,
	pub height: u32,
	pub framerate: f64,
	pub bitrate: u32,
	pub codec: VideoCodec,
	pub timescale: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioSettings {
	pub sample_rate: u32,
	pub channels: u8,
	pub bitrate: u32,
	pub codec: AudioCodec,
	pub timescale: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
	Video,
	Audio,
}

#[derive(Debug, Clone)]
pub struct MediaSegment {
	pub data: Bytes,
	pub ty: MediaType,
	pub keyframe: bool,
	pub timestamp: u64,
}

impl TransmuxResult {
	pub fn into_bytes(self) -> Bytes {
		match self {
			TransmuxResult::InitSegment { data, .. } => data,
			TransmuxResult::MediaSegment(data) => data.data,
		}
	}
}
