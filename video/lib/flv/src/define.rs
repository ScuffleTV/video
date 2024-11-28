use amf0::Amf0Value;
use av1::AV1CodecConfigurationRecord;
use bytes::Bytes;
use h264::AVCDecoderConfigurationRecord;
use h265::HEVCDecoderConfigurationRecord;
use num_derive::FromPrimitive;

#[derive(Debug, Clone, PartialEq)]
/// FLV File
/// Is a container which has a header and a series of tags.
/// Defined in the FLV specification. Chapter 1 - FLV File Format
pub struct Flv {
	pub header: FlvHeader,
	pub tags: Vec<FlvTag>,
}

#[derive(Debug, Clone, PartialEq)]
/// FLV Header
/// Is a 9-byte header which contains information about the FLV file.
/// Defined in the FLV specification. Chapter 1 - The FLV Header
pub struct FlvHeader {
	pub version: u8,
	pub has_audio: bool,
	pub has_video: bool,
	pub data_offset: u32,
	pub extra: Bytes,
}

#[derive(Debug, Clone, PartialEq)]
/// FLV Tag
/// This is a container for the actual media data.
/// Defined in the FLV specification. Chapter 1 - FLV Tags
pub struct FlvTag {
	pub timestamp: u32,
	pub stream_id: u32,
	pub data: FlvTagData,
}

#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq)]
#[repr(u8)]
/// FLV Tag Type
/// Defined in the FLV specification. Chapter 1 - FLV tags
pub enum FlvTagType {
	Audio = 8,
	Video = 9,
	ScriptData = 18,
}

#[derive(Debug, Clone, PartialEq)]
/// FLV Tag Data
/// This is a container for the actual media data.
/// This enum contains the data for the different types of tags.
/// Defined in the FLV specification. Chapter 1 - FLV tags
pub enum FlvTagData {
	/// AudioData defined in the FLV specification. Chapter 1 - FLV Audio Tags
	Audio {
		sound_rate: SoundRate,
		sound_size: SoundSize,
		sound_type: SoundType,
		data: FlvTagAudioData,
	},
	/// VideoData defined in the FLV specification. Chapter 1 - FLV Video Tags
	Video { frame_type: FrameType, data: FlvTagVideoData },
	/// ScriptData defined in the FLV specification. Chapter 1 - FLV Data Tags
	ScriptData { name: String, data: Vec<Amf0Value> },
	/// Data we don't know how to parse
	Unknown { tag_type: u8, data: Bytes },
}

#[derive(Debug, Clone, PartialEq)]
/// FLV Tag Audio Data
/// This is a container for audio data.
/// This enum contains the data for the different types of audio tags.
/// Defined in the FLV specification. Chapter 1 - FLV Audio Tags
pub enum FlvTagAudioData {
	/// AAC Audio Packet defined in the FLV specification. Chapter 1 -
	/// AACAUDIODATA
	Aac(AacPacket),
	/// Data we don't know how to parse
	Unknown { sound_format: u8, data: Bytes },
}

#[derive(Debug, Clone, PartialEq)]
/// AAC Packet
/// This is a container for aac data.
/// This enum contains the data for the different types of aac packets.
/// Defined in the FLV specification. Chapter 1 - AACAUDIODATA
pub enum AacPacket {
	/// AAC Raw
	Raw(Bytes),
	/// AAC Sequence Header
	SequenceHeader(Bytes),
	/// Data we don't know how to parse
	Unknown { aac_packet_type: u8, data: Bytes },
}

#[derive(Debug, Clone, PartialEq)]
/// FLV Tag Video Data
/// This is a container for video data.
/// This enum contains the data for the different types of video tags.
/// Defined in the FLV specification. Chapter 1 - FLV Video Tags
pub enum FlvTagVideoData {
	/// AVC Video Packet defined in the FLV specification. Chapter 1 -
	/// AVCVIDEOPACKET
	Avc(AvcPacket),
	/// Enhanced Packet
	Enhanced(EnhancedPacket),
	/// Data we don't know how to parse
	Unknown { codec_id: u8, data: Bytes },
}

#[derive(Debug, Clone, PartialEq)]
pub enum EnhancedPacket {
	/// Metadata
	Metadata(Bytes),
	/// Sequence End
	SequenceEnd,
	/// Av1 Video Packet
	Av1(Av1Packet),
	/// Hevc (H.265) Video Packet
	Hevc(HevcPacket),
	/// We don't know how to parse it
	Unknown {
		packet_type: u8,
		video_codec: [u8; 4],
		data: Bytes,
	},
}

#[derive(Debug, Clone, PartialEq)]
/// AVC Packet
pub enum AvcPacket {
	/// AVC NALU
	Nalu { composition_time: u32, data: Bytes },
	/// AVC Sequence Header
	SequenceHeader(AVCDecoderConfigurationRecord),
	/// AVC End of Sequence
	EndOfSequence,
	/// AVC Unknown (we don't know how to parse it)
	Unknown {
		avc_packet_type: u8,
		composition_time: u32,
		data: Bytes,
	},
}

#[derive(Debug, Clone, PartialEq)]
/// HEVC Packet
pub enum HevcPacket {
	SequenceStart(HEVCDecoderConfigurationRecord),
	Nalu { composition_time: Option<i32>, data: Bytes },
}

#[derive(Debug, Clone, PartialEq)]
/// AV1 Packet
/// This is a container for av1 data.
/// This enum contains the data for the different types of av1 packets.
pub enum Av1Packet {
	SequenceStart(AV1CodecConfigurationRecord),
	Raw(Bytes),
}

#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum EnhancedPacketType {
	SequenceStart = 0x00,
	CodedFrames = 0x01,
	SequenceEnd = 0x02,
	CodedFramesX = 0x03,
	Metadata = 0x04,
	Mpeg2SequenceStart = 0x05,
}

#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq)]
#[repr(u8)]
/// FLV Sound Codec Id
/// Defined in the FLV specification. Chapter 1 - AudioTags
/// The SoundCodecID indicates the codec used to encode the sound.
pub(crate) enum SoundCodecId {
	LinearPcmPlatformEndian = 0x0,
	Adpcm = 0x1,
	Mp3 = 0x2,
	LinearPcmLittleEndian = 0x3,
	Nellymoser16KhzMono = 0x4,
	Nellymoser8KhzMono = 0x5,
	Nellymoser = 0x6,
	G711ALaw = 0x7,
	G711MuLaw = 0x8,
	Reserved = 0x9,
	Aac = 0xA,
	Speex = 0xB,
	Mp38Khz = 0xE,
	DeviceSpecificSound = 0xF,
}

#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq)]
#[repr(u8)]
/// FLV Sound Rate
/// Defined in the FLV specification. Chapter 1 - AudioTags
/// The SoundRate indicates the sampling rate of the audio data.
pub enum SoundRate {
	Hz5500 = 0x0,
	Hz11000 = 0x1,
	Hz22000 = 0x2,
	Hz44000 = 0x3,
}

#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq)]
#[repr(u8)]
/// FLV Sound Size
/// Defined in the FLV specification. Chapter 1 - AudioTags
/// The SoundSize indicates the size of each sample in the audio data.
pub enum SoundSize {
	Bit8 = 0x0,
	Bit16 = 0x1,
}

#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq)]
#[repr(u8)]
/// FLV Sound Type
/// Defined in the FLV specification. Chapter 1 - AudioTags
/// The SoundType indicates the number of channels in the audio data.
pub enum SoundType {
	Mono = 0x0,
	Stereo = 0x1,
}

#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq)]
#[repr(u8)]
/// FLV AAC Packet Type
/// Defined in the FLV specification. Chapter 1 - AACAUDIODATA
/// The AACPacketType indicates the type of data in the AACAUDIODATA.
pub(crate) enum AacPacketType {
	SeqHdr = 0x0,
	Raw = 0x1,
}

#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq)]
#[repr(u8)]
/// FLV Video Codec ID
/// Defined in the FLV specification. Chapter 1 - VideoTags
/// The codec ID indicates which codec is used to encode the video data.
pub(crate) enum VideoCodecId {
	SorensonH263 = 0x2,
	ScreenVideo = 0x3,
	On2VP6 = 0x4,
	On2VP6WithAlphaChannel = 0x5,
	ScreenVideoVersion2 = 0x6,
	Avc = 0x7,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VideoFourCC {
	Av1,
	Vp9,
	Hevc,
	Unknown([u8; 4]),
}

impl From<[u8; 4]> for VideoFourCC {
	fn from(fourcc: [u8; 4]) -> Self {
		match &fourcc {
			b"av01" => VideoFourCC::Av1,
			b"vp09" => VideoFourCC::Vp9,
			b"hvc1" => VideoFourCC::Hevc,
			_ => VideoFourCC::Unknown(fourcc),
		}
	}
}

impl From<VideoFourCC> for [u8; 4] {
	fn from(fourcc: VideoFourCC) -> Self {
		match fourcc {
			VideoFourCC::Av1 => *b"av01",
			VideoFourCC::Vp9 => *b"vp09",
			VideoFourCC::Hevc => *b"hvc1",
			VideoFourCC::Unknown(fourcc) => fourcc,
		}
	}
}

#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq)]
#[repr(u8)]
/// FLV Frame Type
/// Defined in the FLV specification. Chapter 1 - VideoTags
/// The frame type is used to determine if the video frame is a keyframe, an
/// interframe or disposable interframe.
pub enum FrameType {
	Unknown = 0x0,
	Keyframe = 0x1,
	Interframe = 0x2,
	DisposableInterframe = 0x3,
	GeneratedKeyframe = 0x4,
	VideoInfoOrCommandFrame = 0x5,
	EnhancedMetadata = 0xF,
}

#[derive(Debug, Clone, Copy, FromPrimitive, PartialEq, Eq)]
#[repr(u8)]
/// FLV AVC Packet Type
/// Defined in the FLV specification. Chapter 1 - AVCVIDEODATA
/// The AVC packet type is used to determine if the video data is a sequence
/// header or a NALU.
pub(crate) enum AvcPacketType {
	SeqHdr = 0x0,
	Nalu = 0x1,
	EndOfSequence = 0x2,
}
