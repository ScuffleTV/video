use bytes::Bytes;
use nutype_enum::nutype_enum;

nutype_enum! {
    /// FLV AAC Packet Type
    ///
    /// Defined in the FLV specification. Chapter 1 - AACAUDIODATA
    ///
    /// The AACPacketType indicates the type of data in the AACAUDIODATA.
    pub enum AacPacketType(u8) {
        /// Sequence Header
        SequenceHeader = 0x0,
        /// Raw
        Raw = 0x1,
    }
}

/// AAC Packet
/// This is a container for aac data.
/// This enum contains the data for the different types of aac packets.
/// Defined in the FLV specification. Chapter 1 - AACAUDIODATA
#[derive(Debug, Clone, PartialEq)]
pub enum AacPacket {
    /// AAC Sequence Header
    SequenceHeader(Bytes),
    /// AAC Raw
    Raw(Bytes),
    /// Data we don't know how to parse
    Unknown { aac_packet_type: AacPacketType, data: Bytes },
}

impl AacPacket {
    /// Create a new AAC packet from the given data and packet type
    pub fn new(aac_packet_type: AacPacketType, data: Bytes) -> Self {
        match aac_packet_type {
            AacPacketType::Raw => AacPacket::Raw(data),
            AacPacketType::SequenceHeader => AacPacket::SequenceHeader(data),
            _ => AacPacket::Unknown { aac_packet_type, data },
        }
    }
}

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let cases = [
            (
                AacPacketType::Raw,
                Bytes::from(vec![0, 1, 2, 3]),
                AacPacket::Raw(Bytes::from(vec![0, 1, 2, 3])),
            ),
            (
                AacPacketType::SequenceHeader,
                Bytes::from(vec![0, 1, 2, 3]),
                AacPacket::SequenceHeader(Bytes::from(vec![0, 1, 2, 3])),
            ),
            (
                AacPacketType(0x0),
                Bytes::from(vec![0, 1, 2, 3]),
                AacPacket::SequenceHeader(Bytes::from(vec![0, 1, 2, 3])),
            ),
            (
                AacPacketType(0x1),
                Bytes::from(vec![0, 1, 2, 3]),
                AacPacket::Raw(Bytes::from(vec![0, 1, 2, 3])),
            ),
            (
                AacPacketType(0x2),
                Bytes::from(vec![0, 1, 2, 3]),
                AacPacket::Unknown {
                    aac_packet_type: AacPacketType(0x2),
                    data: Bytes::from(vec![0, 1, 2, 3]),
                },
            ),
            (
                AacPacketType(0x3),
                Bytes::from(vec![0, 1, 2, 3]),
                AacPacket::Unknown {
                    aac_packet_type: AacPacketType(0x3),
                    data: Bytes::from(vec![0, 1, 2, 3]),
                },
            ),
        ];

        for (packet_type, data, expected) in cases {
            let packet = AacPacket::new(packet_type, data.clone());
            assert_eq!(packet, expected);
        }
    }

    #[test]
    fn test_aac_packet_type() {
        assert_eq!(
            format!("{:?}", AacPacketType::SequenceHeader),
            "AacPacketType::SequenceHeader"
        );
        assert_eq!(format!("{:?}", AacPacketType::Raw), "AacPacketType::Raw");
        assert_eq!(format!("{:?}", AacPacketType(0x2)), "AacPacketType(2)");
        assert_eq!(format!("{:?}", AacPacketType(0x3)), "AacPacketType(3)");

        assert_eq!(AacPacketType(0x01), AacPacketType::Raw);
        assert_eq!(AacPacketType(0x00), AacPacketType::SequenceHeader);
    }
}
