use std::io::{
	Read, {self},
};

use amf0::{Amf0Reader, Amf0Value};
use av1::AV1CodecConfigurationRecord;
use byteorder::{BigEndian, ReadBytesExt};
use bytes::{Buf, Bytes};
use bytesio::bytes_reader::BytesCursor;
use h264::AVCDecoderConfigurationRecord;
use h265::HEVCDecoderConfigurationRecord;
use num_traits::FromPrimitive;

use crate::define::Flv;
use crate::{
	AacPacket, AacPacketType, Av1Packet, AvcPacket, AvcPacketType, EnhancedPacket, EnhancedPacketType, FlvDemuxerError,
	FlvHeader, FlvTag, FlvTagAudioData, FlvTagData, FlvTagType, FlvTagVideoData, FrameType, HevcPacket, SoundCodecId,
	SoundRate, SoundSize, SoundType, VideoCodecId, VideoFourCC,
};

impl Flv {
	/// Demux a FLV file.
	pub fn demux(reader: &mut io::Cursor<Bytes>) -> Result<Self, FlvDemuxerError> {
		let header = FlvHeader::demux(reader)?;

		let mut tags = Vec::new();
		while reader.has_remaining() {
			reader.read_u32::<BigEndian>()?; // previous tag size

			if !reader.has_remaining() {
				break;
			}

			let tag = FlvTag::demux(reader)?;
			tags.push(tag);
		}

		Ok(Flv { header, tags })
	}
}

impl FlvHeader {
	pub fn demux(reader: &mut io::Cursor<Bytes>) -> Result<Self, FlvDemuxerError> {
		let mut flv_bytes = [0; 3];
		reader.read_exact(&mut flv_bytes)?;

		if &flv_bytes != b"FLV" {
			return Err(FlvDemuxerError::InvalidFlvHeader);
		}

		let version = reader.read_u8()?;
		let flags = reader.read_u8()?;

		let has_audio = flags & 0b0000_0100 != 0;
		let has_video = flags & 0b0000_0001 != 0;

		let data_offset = reader.read_u32::<BigEndian>()?;

		let remaining = data_offset - reader.position() as u32;
		let extra = reader.read_slice(remaining as usize)?;

		Ok(FlvHeader {
			data_offset,
			has_audio,
			has_video,
			version,
			extra,
		})
	}
}

impl FlvTag {
	pub fn demux(reader: &mut io::Cursor<Bytes>) -> Result<Self, FlvDemuxerError> {
		let tag_type = reader.read_u8()?;
		let data_size = reader.read_u24::<BigEndian>()?;
		let timestamp = reader.read_u24::<BigEndian>()? | (reader.read_u8()? as u32) << 24;
		let stream_id = reader.read_u24::<BigEndian>()?;

		let data = reader.read_slice(data_size as usize)?;

		let data = FlvTagData::demux(tag_type, data)?;

		Ok(FlvTag {
			timestamp,
			stream_id,
			data,
		})
	}
}

impl FlvTagData {
	pub fn demux(tag_type: u8, data: Bytes) -> Result<Self, FlvDemuxerError> {
		let mut reader = io::Cursor::new(data);

		match FlvTagType::from_u8(tag_type) {
			Some(FlvTagType::Audio) => {
				let flags = reader.read_u8()?;

				let sound_format = (flags & 0b1111_0000) >> 4;

				let sound_rate = (flags & 0b0000_1100) >> 2;
				let sound_rate =
					SoundRate::from_u8(sound_rate).ok_or_else(|| FlvDemuxerError::InvalidSoundRate(sound_rate))?;

				let sound_size = (flags & 0b0000_0010) >> 1;
				let sound_size =
					SoundSize::from_u8(sound_size).ok_or_else(|| FlvDemuxerError::InvalidSoundSize(sound_size))?;

				let sound_type = flags & 0b0000_0001;
				let sound_type =
					SoundType::from_u8(sound_type).ok_or_else(|| FlvDemuxerError::InvalidSoundType(sound_type))?;

				let data = FlvTagAudioData::demux(sound_format, &mut reader)?;

				Ok(FlvTagData::Audio {
					sound_rate,
					sound_size,
					sound_type,
					data,
				})
			}
			Some(FlvTagType::Video) => {
				let flags = reader.read_u8()?;
				let mut frame_type = flags >> 4;

				let mut is_enhanced = false;
				let codec_id = flags & 0b0000_1111;

				if frame_type & 0b1000 != 0 {
					// Enhanced Flv Tag
					frame_type &= 0b0111;
					is_enhanced = true;

					if codec_id == EnhancedPacketType::Metadata as u8 {
						frame_type = FrameType::EnhancedMetadata as u8;
					}
				}

				let frame_type =
					FrameType::from_u8(frame_type).ok_or_else(|| FlvDemuxerError::InvalidFrameType(frame_type))?;

				Ok(FlvTagData::Video {
					frame_type,
					data: if is_enhanced {
						FlvTagVideoData::demux_enhanced(codec_id, &mut reader)?
					} else {
						FlvTagVideoData::demux(codec_id, &mut reader)?
					},
				})
			}
			Some(FlvTagType::ScriptData) => {
				let values = Amf0Reader::new(reader.extract_remaining()).read_all()?;

				let name = match values.first() {
					Some(Amf0Value::String(name)) => name,
					_ => return Err(FlvDemuxerError::InvalidScriptDataName),
				};

				Ok(FlvTagData::ScriptData {
					name: name.clone(),
					data: values.into_iter().skip(1).collect(),
				})
			}
			None => Ok(FlvTagData::Unknown {
				tag_type,
				data: reader.extract_remaining(),
			}),
		}
	}
}

impl FlvTagAudioData {
	pub fn demux(sound_format: u8, reader: &mut io::Cursor<Bytes>) -> Result<Self, FlvDemuxerError> {
		match SoundCodecId::from_u8(sound_format) {
			Some(SoundCodecId::Aac) => {
				let aac_packet_type = reader.read_u8()?;
				Ok(Self::Aac(AacPacket::demux(aac_packet_type, reader)?))
			}
			_ => Ok(Self::Unknown {
				sound_format,
				data: reader.extract_remaining(),
			}),
		}
	}
}

impl AacPacket {
	pub fn demux(aac_packet_type: u8, reader: &mut io::Cursor<Bytes>) -> Result<Self, FlvDemuxerError> {
		match AacPacketType::from_u8(aac_packet_type) {
			Some(AacPacketType::SeqHdr) => Ok(Self::SequenceHeader(reader.extract_remaining())),
			Some(AacPacketType::Raw) => Ok(Self::Raw(reader.extract_remaining())),
			_ => Ok(Self::Unknown {
				aac_packet_type,
				data: reader.extract_remaining(),
			}),
		}
	}
}

impl FlvTagVideoData {
	pub fn demux(codec_id: u8, reader: &mut io::Cursor<Bytes>) -> Result<Self, FlvDemuxerError> {
		match VideoCodecId::from_u8(codec_id) {
			Some(VideoCodecId::Avc) => {
				let avc_packet_type = reader.read_u8()?;
				Ok(Self::Avc(AvcPacket::demux(avc_packet_type, reader)?))
			}
			_ => Ok(Self::Unknown {
				codec_id,
				data: reader.extract_remaining(),
			}),
		}
	}

	pub fn demux_enhanced(packet_type: u8, reader: &mut io::Cursor<Bytes>) -> Result<Self, FlvDemuxerError> {
		// In the enhanced spec the codec id is the packet type
		let packet_type = EnhancedPacketType::from_u8(packet_type)
			.ok_or_else(|| FlvDemuxerError::InvalidEnhancedPacketType(packet_type))?;
		let mut video_codec = [0; 4];
		reader.read_exact(&mut video_codec)?;
		let video_codec = VideoFourCC::from(video_codec);

		match packet_type {
			EnhancedPacketType::SequenceEnd => {
				return Ok(Self::Enhanced(EnhancedPacket::SequenceEnd));
			}
			EnhancedPacketType::Metadata => {
				return Ok(Self::Enhanced(EnhancedPacket::Metadata(reader.extract_remaining())));
			}
			_ => {}
		}

		match (video_codec, packet_type) {
			(VideoFourCC::Av1, EnhancedPacketType::SequenceStart) => Ok(Self::Enhanced(EnhancedPacket::Av1(
				Av1Packet::SequenceStart(AV1CodecConfigurationRecord::demux(reader)?),
			))),
			(VideoFourCC::Av1, EnhancedPacketType::CodedFrames) => Ok(Self::Enhanced(EnhancedPacket::Av1(Av1Packet::Raw(
				reader.extract_remaining(),
			)))),
			(VideoFourCC::Hevc, EnhancedPacketType::SequenceStart) => Ok(Self::Enhanced(EnhancedPacket::Hevc(
				HevcPacket::SequenceStart(HEVCDecoderConfigurationRecord::demux(reader)?),
			))),
			(VideoFourCC::Hevc, EnhancedPacketType::CodedFrames) => {
				let composition_time = reader.read_i24::<BigEndian>()?;
				Ok(Self::Enhanced(EnhancedPacket::Hevc(HevcPacket::Nalu {
					composition_time: Some(composition_time),
					data: reader.extract_remaining(),
				})))
			}
			(VideoFourCC::Hevc, EnhancedPacketType::CodedFramesX) => {
				Ok(Self::Enhanced(EnhancedPacket::Hevc(HevcPacket::Nalu {
					composition_time: None,
					data: reader.extract_remaining(),
				})))
			}
			_ => Ok(Self::Enhanced(EnhancedPacket::Unknown {
				packet_type: packet_type as u8,
				video_codec: video_codec.into(),
				data: reader.extract_remaining(),
			})),
		}
	}
}

impl AvcPacket {
	pub fn demux(avc_packet_type: u8, reader: &mut io::Cursor<Bytes>) -> Result<Self, FlvDemuxerError> {
		match AvcPacketType::from_u8(avc_packet_type) {
			Some(AvcPacketType::SeqHdr) => {
				reader.read_u24::<BigEndian>()?; // composition time (always 0)
				Ok(Self::SequenceHeader(AVCDecoderConfigurationRecord::demux(reader)?))
			}
			Some(AvcPacketType::Nalu) => Ok(Self::Nalu {
				composition_time: reader.read_u24::<BigEndian>()?,
				data: reader.extract_remaining(),
			}),
			Some(AvcPacketType::EndOfSequence) => Ok(Self::EndOfSequence),
			_ => Ok(Self::Unknown {
				avc_packet_type,
				composition_time: reader.read_u24::<BigEndian>()?,
				data: reader.extract_remaining(),
			}),
		}
	}
}
