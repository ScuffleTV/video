use std::io::{self, Read};

use byteorder::{BigEndian, ReadBytesExt};
use bytes::Bytes;
use nutype_enum::nutype_enum;
use scuffle_av1::{AV1CodecConfigurationRecord, AV1VideoDescriptor};
use scuffle_bytes_util::BytesCursorExt;
use scuffle_h265::HEVCDecoderConfigurationRecord;

use super::av1::Av1Packet;
use super::avc::AvcPacket;
use super::hevc::HevcPacket;

nutype_enum! {
    /// FLV Frame Type
    /// This enum represents the different types of frames in a FLV file.
    /// Defined by:
    /// - video_file_format_spec_v10.pdf (Chapter 1 - The FLV File Format - Video tags)
    /// - video_file_format_spec_v10_1.pdf (Annex E.4.3.1 - VIDEODATA)
    pub enum FrameType(u8) {
        /// A keyframe is a frame that is a complete representation of the video content.
        Keyframe = 1,
        /// An interframe is a frame that is a partial representation of the video content.
        Interframe = 2,
        /// A disposable interframe is a frame that is a partial representation of the video content, but is not required to be displayed. (h263 only)
        DisposableInterframe = 3,
        /// A generated keyframe is a frame that is a complete representation of the video content, but is not a keyframe. (reserved for server use only)
        GeneratedKeyframe = 4,
        /// A video info or command frame is a frame that contains video information or commands.
        /// If the frame is this type, the body will be a CommandPacket
        VideoInfoOrCommandFrame = 5,
    }
}

/// FLV Tag Video Header
/// This is a container for video data.
/// This enum contains the data for the different types of video tags.
/// Defined by:
/// - video_file_format_spec_v10.pdf (Chapter 1 - The FLV File Format - Video tags)
/// - video_file_format_spec_v10_1.pdf (Annex E.4.3.1 - VIDEODATA)
#[derive(Debug, Clone, PartialEq)]
pub struct VideoTagHeader {
    /// The frame type of the video data. (4 bits)
    pub frame_type: FrameType,
    /// The body of the video data.
    pub body: VideoTagBody,
}

impl VideoTagHeader {
    /// Demux a video data from the given reader
    pub fn demux(reader: &mut io::Cursor<Bytes>) -> io::Result<Self> {
        let byte = reader.read_u8()?;
        let enhanced = (byte & 0b1000_0000) != 0;
        let frame_type_byte = (byte >> 4) & 0b0111;
        let packet_type_byte = byte & 0b0000_1111;
        let frame_type = FrameType::from(frame_type_byte);
        let body = if frame_type == FrameType::VideoInfoOrCommandFrame {
            let command_packet = CommandPacket::from(reader.read_u8()?);
            VideoTagBody::Command(command_packet)
        } else {
            VideoTagBody::demux(VideoPacketType::new(packet_type_byte, enhanced), reader)?
        };

        Ok(VideoTagHeader { frame_type, body })
    }
}

nutype_enum! {
    /// FLV Video Codec ID
    ///
    /// Denotes the different types of video codecs that can be used in a FLV file.
    /// This is a legacy enum for older codecs; for modern codecs, the [`EnhancedPacketType`] is used which uses a [`VideoFourCC`] identifier.
    ///
    /// Defined by:
    /// - video_file_format_spec_v10.pdf (Chapter 1 - The FLV File Format - Video tags)
    /// - video_file_format_spec_v10_1.pdf (Annex E.4.3.1 - VIDEODATA)
    pub enum VideoCodecId(u8) {
        /// Sorenson H.263
        SorensonH263 = 2,
        /// Screen Video
        ScreenVideo = 3,
        /// On2 VP6
        On2VP6 = 4,
        /// On2 VP6 with alpha channel
        On2VP6WithAlphaChannel = 5,
        /// Screen Video Version 2
        ScreenVideoVersion2 = 6,
        /// AVC (H.264)
        Avc = 7,
    }
}

/// FLV Tag Video Data Body
///
/// This is a container for video data.
/// This enum contains the data for the different types of video tags.
///
/// Defined by:
/// - video_file_format_spec_v10.pdf (Chapter 1 - The FLV File Format - Video
///   tags)
/// - video_file_format_spec_v10_1.pdf (Annex E.4.3.1 - VIDEODATA)
#[derive(Debug, Clone, PartialEq)]
pub enum VideoTagBody {
    /// AVC Video Packet (H.264)
    /// When [`VideoPacketType::CodecId`] is [`VideoCodecId::Avc`]
    Avc(AvcPacket),
    /// Enhanced Packet (AV1, H.265, etc.)
    /// When [`VideoPacketType::Enhanced`] is used
    Enhanced(EnhancedPacket),
    /// Command Frame (VideoInfo or Command)
    /// When [`FrameType::VideoInfoOrCommandFrame`] is used
    Command(CommandPacket),
    /// Data we don't know how to parse
    Unknown { codec_id: VideoCodecId, data: Bytes },
}

nutype_enum! {
    /// FLV Command Packet
    /// Defined by:
    /// - video_file_format_spec_v10.pdf (Chapter 1 - The FLV File Format - Video tags)
    /// - video_file_format_spec_v10_1.pdf (Annex E.4.3.1 - VIDEODATA)
    pub enum CommandPacket(u8) {
        /// Start of client seeking, when FrameType is 5
        StartOfClientSeeking = 1,
        /// End of client seeking, when FrameType is 5
        EndOfClientSeeking = 2,
    }
}

/// A wrapper enum for the different types of video packets that can be used in
/// a FLV file.
///
/// Used to construct a [`VideoTagBody`].
///
/// See:
/// - [`VideoCodecId`]
/// - [`EnhancedPacketType`]
/// - [`VideoTagBody`]
#[derive(Debug, Clone, PartialEq, Copy, Eq, PartialOrd, Ord, Hash)]
pub enum VideoPacketType {
    /// Codec ID (legacy)
    CodecId(VideoCodecId),
    /// Enhanced (modern)
    Enhanced(EnhancedPacketType),
}

impl VideoPacketType {
    pub fn new(byte: u8, enhanced: bool) -> Self {
        if enhanced {
            Self::Enhanced(EnhancedPacketType::from(byte))
        } else {
            Self::CodecId(VideoCodecId::from(byte))
        }
    }
}

impl VideoTagBody {
    /// Demux a video packet from the given reader.
    /// The reader will consume all the data from the reader.
    pub fn demux(packet_type: VideoPacketType, reader: &mut io::Cursor<Bytes>) -> io::Result<Self> {
        match packet_type {
            VideoPacketType::CodecId(codec_id) => match codec_id {
                VideoCodecId::Avc => Ok(VideoTagBody::Avc(AvcPacket::demux(reader)?)),
                _ => Ok(VideoTagBody::Unknown {
                    codec_id,
                    data: reader.extract_remaining(),
                }),
            },
            VideoPacketType::Enhanced(packet_type) => {
                let mut video_codec = [0; 4];
                reader.read_exact(&mut video_codec)?;
                let video_codec = VideoFourCC::from(video_codec);

                match packet_type {
                    EnhancedPacketType::SequenceEnd => {
                        return Ok(VideoTagBody::Enhanced(EnhancedPacket::SequenceEnd { video_codec }))
                    }
                    EnhancedPacketType::Metadata => {
                        return Ok(VideoTagBody::Enhanced(EnhancedPacket::Metadata {
                            video_codec,
                            data: reader.extract_remaining(),
                        }))
                    }
                    _ => {}
                }

                match (video_codec, packet_type) {
                    (VideoFourCC::Av1, EnhancedPacketType::SequenceStart) => Ok(VideoTagBody::Enhanced(
                        EnhancedPacket::Av1(Av1Packet::SequenceStart(AV1CodecConfigurationRecord::demux(reader)?)),
                    )),
                    (VideoFourCC::Av1, EnhancedPacketType::Mpeg2SequenceStart) => {
                        Ok(VideoTagBody::Enhanced(EnhancedPacket::Av1(Av1Packet::SequenceStart(
                            AV1VideoDescriptor::demux(reader)?.codec_configuration_record,
                        ))))
                    }
                    (VideoFourCC::Av1, EnhancedPacketType::CodedFrames) => Ok(VideoTagBody::Enhanced(EnhancedPacket::Av1(
                        Av1Packet::Raw(reader.extract_remaining()),
                    ))),
                    (VideoFourCC::Hevc, EnhancedPacketType::SequenceStart) => Ok(VideoTagBody::Enhanced(
                        EnhancedPacket::Hevc(HevcPacket::SequenceStart(HEVCDecoderConfigurationRecord::demux(reader)?)),
                    )),
                    (VideoFourCC::Hevc, EnhancedPacketType::CodedFrames) => {
                        Ok(VideoTagBody::Enhanced(EnhancedPacket::Hevc(HevcPacket::Nalu {
                            composition_time: Some(reader.read_i24::<BigEndian>()?),
                            data: reader.extract_remaining(),
                        })))
                    }
                    (VideoFourCC::Hevc, EnhancedPacketType::CodedFramesX) => {
                        Ok(VideoTagBody::Enhanced(EnhancedPacket::Hevc(HevcPacket::Nalu {
                            composition_time: None,
                            data: reader.extract_remaining(),
                        })))
                    }
                    _ => Ok(VideoTagBody::Enhanced(EnhancedPacket::Unknown {
                        packet_type,
                        video_codec,
                        data: reader.extract_remaining(),
                    })),
                }
            }
        }
    }
}

/// An Enhanced FLV Packet
///
/// This is a container for enhanced video packets.
/// The enchanced spec adds modern codecs to the FLV file format.
///
/// Defined by:
/// - enhanced_rtmp-v1.pdf (Defining Additional Video Codecs)
/// - enhanced_rtmp-v2.pdf (Enhanced Video)
#[derive(Debug, Clone, PartialEq)]
pub enum EnhancedPacket {
    /// Metadata
    Metadata { video_codec: VideoFourCC, data: Bytes },
    /// Sequence End
    SequenceEnd { video_codec: VideoFourCC },
    /// Av1 Video Packet
    Av1(Av1Packet),
    /// Hevc (H.265) Video Packet
    Hevc(HevcPacket),
    /// We don't know how to parse it
    Unknown {
        packet_type: EnhancedPacketType,
        video_codec: VideoFourCC,
        data: Bytes,
    },
}

nutype_enum! {
    /// FLV Video FourCC
    ///
    /// Denotes the different types of video codecs that can be used in a FLV file.
    ///
    /// Defined by:
    /// - enhanced_rtmp-v1.pdf (Defining Additional Video Codecs)
    /// - enhanced_rtmp-v2.pdf (Enhanced Video)
    pub enum VideoFourCC([u8; 4]) {
        /// AV1
        Av1 = *b"av01",
        /// VP9
        Vp9 = *b"vp09",
        /// HEVC (H.265)
        Hevc = *b"hvc1",
    }
}

nutype_enum! {
    /// Enhanced Packet Type
    ///
    /// The type of packet in an enhanced FLV file.
    ///
    /// Defined by:
    /// - enhanced_rtmp-v1.pdf (Defining Additional Video Codecs)
    /// - enhanced_rtmp-v2.pdf (Enhanced Video)
    pub enum EnhancedPacketType(u8) {
        /// Sequence Start
        SequenceStart = 0,
        /// Coded Frames
        CodedFrames = 1,
        /// Sequence End
        SequenceEnd = 2,
        /// Coded Frames X
        CodedFramesX = 3,
        /// Metadata
        Metadata = 4,
        /// MPEG-2 Sequence Start
        Mpeg2SequenceStart = 5,
    }
}

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use super::*;
    use crate::avc::AvcPacketType;

    #[test]
    fn test_video_fourcc() {
        let cases = [
            (VideoFourCC::Av1, *b"av01", "VideoFourCC::Av1"),
            (VideoFourCC::Vp9, *b"vp09", "VideoFourCC::Vp9"),
            (VideoFourCC::Hevc, *b"hvc1", "VideoFourCC::Hevc"),
            (VideoFourCC(*b"av02"), *b"av02", "VideoFourCC([97, 118, 48, 50])"),
        ];

        for (expected, bytes, name) in cases {
            assert_eq!(VideoFourCC::from(bytes), expected);
            assert_eq!(format!("{:?}", VideoFourCC::from(bytes)), name);
        }
    }

    #[test]
    fn test_enhanced_packet_type() {
        let cases = [
            (EnhancedPacketType::SequenceStart, 0, "EnhancedPacketType::SequenceStart"),
            (EnhancedPacketType::CodedFrames, 1, "EnhancedPacketType::CodedFrames"),
            (EnhancedPacketType::SequenceEnd, 2, "EnhancedPacketType::SequenceEnd"),
            (EnhancedPacketType::CodedFramesX, 3, "EnhancedPacketType::CodedFramesX"),
            (EnhancedPacketType::Metadata, 4, "EnhancedPacketType::Metadata"),
            (
                EnhancedPacketType::Mpeg2SequenceStart,
                5,
                "EnhancedPacketType::Mpeg2SequenceStart",
            ),
            (EnhancedPacketType(6), 6, "EnhancedPacketType(6)"),
            (EnhancedPacketType(7), 7, "EnhancedPacketType(7)"),
        ];

        for (expected, value, name) in cases {
            assert_eq!(EnhancedPacketType::from(value), expected);
            assert_eq!(format!("{:?}", EnhancedPacketType::from(value)), name);
        }
    }

    #[test]
    fn test_frame_type() {
        let cases = [
            (FrameType::Keyframe, 1, "FrameType::Keyframe"),
            (FrameType::Interframe, 2, "FrameType::Interframe"),
            (FrameType::DisposableInterframe, 3, "FrameType::DisposableInterframe"),
            (FrameType::GeneratedKeyframe, 4, "FrameType::GeneratedKeyframe"),
            (FrameType::VideoInfoOrCommandFrame, 5, "FrameType::VideoInfoOrCommandFrame"),
            (FrameType(6), 6, "FrameType(6)"),
            (FrameType(7), 7, "FrameType(7)"),
        ];

        for (expected, value, name) in cases {
            assert_eq!(FrameType::from(value), expected);
            assert_eq!(format!("{:?}", FrameType::from(value)), name);
        }
    }

    #[test]
    fn test_video_codec_id() {
        let cases = [
            (VideoCodecId::SorensonH263, 2, "VideoCodecId::SorensonH263"),
            (VideoCodecId::ScreenVideo, 3, "VideoCodecId::ScreenVideo"),
            (VideoCodecId::On2VP6, 4, "VideoCodecId::On2VP6"),
            (
                VideoCodecId::On2VP6WithAlphaChannel,
                5,
                "VideoCodecId::On2VP6WithAlphaChannel",
            ),
            (VideoCodecId::ScreenVideoVersion2, 6, "VideoCodecId::ScreenVideoVersion2"),
            (VideoCodecId::Avc, 7, "VideoCodecId::Avc"),
            (VideoCodecId(10), 10, "VideoCodecId(10)"),
            (VideoCodecId(11), 11, "VideoCodecId(11)"),
            (VideoCodecId(15), 15, "VideoCodecId(15)"),
        ];

        for (expected, value, name) in cases {
            assert_eq!(VideoCodecId::from(value), expected);
            assert_eq!(format!("{:?}", VideoCodecId::from(value)), name);
        }
    }

    #[test]
    fn test_command_packet() {
        let cases = [
            (CommandPacket::StartOfClientSeeking, 1, "CommandPacket::StartOfClientSeeking"),
            (CommandPacket::EndOfClientSeeking, 2, "CommandPacket::EndOfClientSeeking"),
            (CommandPacket(3), 3, "CommandPacket(3)"),
            (CommandPacket(4), 4, "CommandPacket(4)"),
        ];

        for (expected, value, name) in cases {
            assert_eq!(CommandPacket::from(value), expected);
            assert_eq!(format!("{:?}", CommandPacket::from(value)), name);
        }
    }

    #[test]
    fn test_video_packet_type() {
        let cases = [
            (1, true, VideoPacketType::Enhanced(EnhancedPacketType::CodedFrames)),
            (7, false, VideoPacketType::CodecId(VideoCodecId::Avc)),
        ];

        for (value, enhanced, expected) in cases {
            assert_eq!(VideoPacketType::new(value, enhanced), expected);
        }
    }

    #[test]
    fn test_video_data_body_metadata() {
        let mut reader = io::Cursor::new(Bytes::from_static(&[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]));
        let packet_type = VideoPacketType::new(0x4, true);
        let body = VideoTagBody::demux(packet_type, &mut reader).unwrap();
        assert_eq!(
            body,
            VideoTagBody::Enhanced(EnhancedPacket::Metadata {
                video_codec: VideoFourCC([1, 2, 3, 4]),
                data: Bytes::from_static(&[0x05, 0x06, 0x07, 0x08]),
            })
        );
    }

    #[test]
    fn test_video_data_body_avc() {
        let mut reader = io::Cursor::new(Bytes::from_static(&[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]));
        let packet_type = VideoPacketType::new(0x7, false);
        let body = VideoTagBody::demux(packet_type, &mut reader).unwrap();
        assert_eq!(
            body,
            VideoTagBody::Avc(AvcPacket::Nalu {
                // first byte is the avc packet type (in this case, 1 = nalu)
                composition_time: 0x020304,
                data: Bytes::from_static(&[0x05, 0x06, 0x07, 0x08]),
            })
        );

        let mut reader = io::Cursor::new(Bytes::from_static(&[0x05, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]));
        let packet_type = VideoPacketType::new(0x7, false);
        let body = VideoTagBody::demux(packet_type, &mut reader).unwrap();
        assert_eq!(
            body,
            VideoTagBody::Avc(AvcPacket::Unknown {
                avc_packet_type: AvcPacketType(5),
                composition_time: 0x020304,
                data: Bytes::from_static(&[0x05, 0x06, 0x07, 0x08]),
            })
        );
    }

    #[test]
    fn test_video_data_body_hevc() {
        let mut reader = io::Cursor::new(Bytes::from_static(&[
            b'h', b'v', b'c', b'1', // video codec
            0x01, 0x02, 0x03, 0x04, // data
            0x05, 0x06, 0x07, 0x08, // data
        ]));
        let packet_type = VideoPacketType::new(0x3, true);
        let body = VideoTagBody::demux(packet_type, &mut reader).unwrap();
        assert_eq!(
            body,
            VideoTagBody::Enhanced(EnhancedPacket::Hevc(HevcPacket::Nalu {
                composition_time: None,
                data: Bytes::from_static(&[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]),
            }))
        );

        let mut reader = io::Cursor::new(Bytes::from_static(&[
            b'h', b'v', b'c', b'2', // video codec
            0x01, 0x02, 0x03, 0x04, // data
            0x05, 0x06, 0x07, 0x08, // data
        ]));
        let packet_type = VideoPacketType::new(0x3, true);
        let body = VideoTagBody::demux(packet_type, &mut reader).unwrap();
        assert_eq!(
            body,
            VideoTagBody::Enhanced(EnhancedPacket::Unknown {
                packet_type: EnhancedPacketType::CodedFramesX,
                video_codec: VideoFourCC([b'h', b'v', b'c', b'2']),
                data: Bytes::from_static(&[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]),
            })
        );
    }

    #[test]
    fn test_video_data_body_av1() {
        let mut reader = io::Cursor::new(Bytes::from_static(&[
            b'a', b'v', b'0', b'1', // video codec
            0x01, 0x02, 0x03, 0x04, // data
            0x05, 0x06, 0x07, 0x08, // data
        ]));
        let packet_type = VideoPacketType::new(0x04, true);
        let body = VideoTagBody::demux(packet_type, &mut reader).unwrap();
        assert_eq!(
            body,
            VideoTagBody::Enhanced(EnhancedPacket::Metadata {
                video_codec: VideoFourCC([b'a', b'v', b'0', b'1']),
                data: Bytes::from_static(&[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]),
            })
        );
    }

    #[test]
    fn test_video_data_command_packet() {
        let mut reader = io::Cursor::new(Bytes::from_static(&[
            0b01010000, // frame type (5)
            0x01,       // command packet
        ]));
        let body = VideoTagHeader::demux(&mut reader).unwrap();
        assert_eq!(
            body,
            VideoTagHeader {
                frame_type: FrameType::VideoInfoOrCommandFrame,
                body: VideoTagBody::Command(CommandPacket::StartOfClientSeeking),
            }
        );
    }

    #[test]
    fn test_video_data_demux_enhanced() {
        let mut reader = io::Cursor::new(Bytes::from_static(&[
            0b10010010, // enhanced + keyframe
            b'a', b'v', b'0', b'1', // video codec
        ]));
        let body = VideoTagHeader::demux(&mut reader).unwrap();
        assert_eq!(
            body,
            VideoTagHeader {
                frame_type: FrameType::Keyframe,
                body: VideoTagBody::Enhanced(EnhancedPacket::SequenceEnd {
                    video_codec: VideoFourCC([b'a', b'v', b'0', b'1']),
                }),
            }
        );
    }

    #[test]
    fn test_video_data_demux_h263() {
        let mut reader = io::Cursor::new(Bytes::from_static(&[
            0b00010010, // enhanced + keyframe
            0, 1, 2, 3, // data
        ]));
        let body = VideoTagHeader::demux(&mut reader).unwrap();
        assert_eq!(
            body,
            VideoTagHeader {
                frame_type: FrameType::Keyframe,
                body: VideoTagBody::Unknown {
                    codec_id: VideoCodecId::SorensonH263,
                    data: Bytes::from_static(&[0, 1, 2, 3]),
                },
            }
        );
    }

    #[test]
    fn test_av1_mpeg2_sequence_start() {
        let mut reader = io::Cursor::new(Bytes::from_static(&[
            0b10010101, // enhanced + keyframe
            b'a', b'v', b'0', b'1', // video codec
            0x80, 0x4, 129, 13, 12, 0, 10, 15, 0, 0, 0, 106, 239, 191, 225, 188, 2, 25, 144, 16, 16, 16, 64,
        ]));

        let body = VideoTagHeader::demux(&mut reader).unwrap();
        assert_eq!(
            body,
            VideoTagHeader {
                frame_type: FrameType::Keyframe,
                body: VideoTagBody::Enhanced(EnhancedPacket::Av1(Av1Packet::SequenceStart(AV1CodecConfigurationRecord {
                    seq_profile: 0,
                    seq_level_idx_0: 13,
                    seq_tier_0: false,
                    high_bitdepth: false,
                    twelve_bit: false,
                    monochrome: false,
                    chroma_subsampling_x: true,
                    chroma_subsampling_y: true,
                    chroma_sample_position: 0,
                    hdr_wcg_idc: 0,
                    initial_presentation_delay_minus_one: None,
                    config_obu: Bytes::from_static(b"\n\x0f\0\0\0j\xef\xbf\xe1\xbc\x02\x19\x90\x10\x10\x10@"),
                }))),
            }
        );
    }
}
