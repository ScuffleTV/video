use bytes::Bytes;
use scuffle_h265::HEVCDecoderConfigurationRecord;

/// HEVC Packet
#[derive(Debug, Clone, PartialEq)]
pub enum HevcPacket {
    /// HEVC Sequence Start
    SequenceStart(HEVCDecoderConfigurationRecord),
    /// HEVC NALU
    Nalu { composition_time: Option<i32>, data: Bytes },
}
