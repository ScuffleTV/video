use std::io;

use byteorder::{BigEndian, ReadBytesExt};
use bytes::Bytes;
use nutype_enum::nutype_enum;
use scuffle_bytes_util::BytesCursorExt;
use scuffle_h264::AVCDecoderConfigurationRecord;

/// AVC Packet
#[derive(Debug, Clone, PartialEq)]
pub enum AvcPacket {
    /// AVC NALU
    Nalu { composition_time: u32, data: Bytes },
    /// AVC Sequence Header
    SequenceHeader(AVCDecoderConfigurationRecord),
    /// AVC End of Sequence
    EndOfSequence,
    /// AVC Unknown (we don't know how to parse it)
    Unknown {
        avc_packet_type: AvcPacketType,
        composition_time: u32,
        data: Bytes,
    },
}

impl AvcPacket {
    pub fn demux(reader: &mut io::Cursor<Bytes>) -> io::Result<Self> {
        let avc_packet_type = AvcPacketType::from(reader.read_u8()?);
        let composition_time = reader.read_u24::<BigEndian>()?;

        match avc_packet_type {
            AvcPacketType::SeqHdr => Ok(Self::SequenceHeader(AVCDecoderConfigurationRecord::demux(reader)?)),
            AvcPacketType::Nalu => Ok(Self::Nalu {
                composition_time,
                data: reader.extract_remaining(),
            }),
            AvcPacketType::EndOfSequence => Ok(Self::EndOfSequence),
            _ => Ok(Self::Unknown {
                avc_packet_type,
                composition_time,
                data: reader.extract_remaining(),
            }),
        }
    }
}

nutype_enum! {
    /// FLV AVC Packet Type
    /// Defined in the FLV specification. Chapter 1 - AVCVIDEODATA
    /// The AVC packet type is used to determine if the video data is a sequence
    /// header or a NALU.
    pub enum AvcPacketType(u8) {
        SeqHdr = 0,
        Nalu = 1,
        EndOfSequence = 2,
    }
}
