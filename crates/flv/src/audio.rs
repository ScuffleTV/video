use std::io;

use byteorder::ReadBytesExt;
use bytes::Bytes;
use nutype_enum::nutype_enum;
use scuffle_bytes_util::BytesCursorExt;

use super::aac::{AacPacket, AacPacketType};

/// FLV Tag Audio Data
///
/// This is the container for the audio data.
///
/// Defined by:
/// - video_file_format_spec_v10.pdf (Chapter 1 - The FLV File Format - Audio tags)
/// - video_file_format_spec_v10_1.pdf (Annex E.4.2.1 - AUDIODATA)
#[derive(Debug, Clone, PartialEq)]
pub struct AudioData {
    /// The sound rate of the audio data. (2 bits)
    pub sound_rate: SoundRate,
    /// The sound size of the audio data. (1 bit)
    pub sound_size: SoundSize,
    /// The sound type of the audio data. (1 bit)
    pub sound_type: SoundType,
    /// The body of the audio data.
    pub body: AudioDataBody,
}

impl AudioData {
    pub fn demux(reader: &mut io::Cursor<Bytes>) -> io::Result<Self> {
        let byte = reader.read_u8()?;
        // SoundFormat is the first 4 bits of the byte
        let sound_format = SoundFormat::from(byte >> 4);
        // SoundRate is the next 2 bits of the byte
        let sound_rate = SoundRate::from((byte >> 2) & 0b11);
        // SoundSize is the next bit of the byte
        let sound_size = SoundSize::from((byte >> 1) & 0b1);
        // SoundType is the last bit of the byte
        let sound_type = SoundType::from(byte & 0b1);

        // Now we can demux the body of the audio data
        let body = AudioDataBody::demux(sound_format, reader)?;

        Ok(AudioData {
            sound_rate,
            sound_size,
            sound_type,
            body,
        })
    }
}

nutype_enum! {
    /// FLV Sound Format
    ///
    /// Denotes the type of the underlying data packet
    ///
    /// Defined by:
    /// - video_file_format_spec_v10.pdf (Chapter 1 - The FLV File Format - Audio tags)
    /// - video_file_format_spec_v10_1.pdf (Annex E.4.2.1 - AUDIODATA)
    pub enum SoundFormat(u8) {
        /// Linear PCM, platform endian
        LinearPcmPlatformEndian = 0,
        /// ADPCM
        Adpcm = 1,
        /// MP3
        Mp3 = 2,
        /// Linear PCM, little endian
        LinearPcmLittleEndian = 3,
        /// Nellymoser 16Khz Mono
        Nellymoser16KhzMono = 4,
        /// Nellymoser 8Khz Mono
        Nellymoser8KhzMono = 5,
        /// Nellymoser
        Nellymoser = 6,
        /// G.711 A-Law logarithmic PCM
        G711ALaw = 7,
        /// G.711 Mu-Law logarithmic PCM
        G711MuLaw = 8,
        /// AAC
        Aac = 10,
        /// Speex
        Speex = 11,
        /// Mp3 8Khz
        Mp38Khz = 14,
        /// Device specific sound
        DeviceSpecificSound = 15,
    }
}

/// FLV Tag Audio Data Body
///
/// This is the container for the audio data body.
///
/// Defined by:
/// - video_file_format_spec_v10.pdf (Chapter 1 - The FLV File Format - Audio tags)
/// - video_file_format_spec_v10_1.pdf (Annex E.4.2.1 - AUDIODATA)
#[derive(Debug, Clone, PartialEq)]
pub enum AudioDataBody {
    /// AAC Audio Packet
    Aac(AacPacket),
    /// Some other audio format we don't know how to parse
    Unknown { sound_format: SoundFormat, data: Bytes },
}

impl AudioDataBody {
    /// Demux the audio data body from the given reader
    ///
    /// The reader will be entirely consumed.
    pub fn demux(sound_format: SoundFormat, reader: &mut io::Cursor<Bytes>) -> io::Result<Self> {
        match sound_format {
            SoundFormat::Aac => {
                // For some reason the spec adds a specific byte before the AAC data.
                // This byte is the AAC packet type.
                let aac_packet_type = AacPacketType::from(reader.read_u8()?);
                Ok(Self::Aac(AacPacket::new(aac_packet_type, reader.extract_remaining())))
            }
            _ => Ok(Self::Unknown {
                sound_format,
                data: reader.extract_remaining(),
            }),
        }
    }
}

nutype_enum! {
    /// FLV Sound Rate
    ///
    /// Denotes the sampling rate of the audio data.
    ///
    /// Defined by:
    /// - video_file_format_spec_v10.pdf (Chapter 1 - The FLV File Format - Audio tags)
    /// - video_file_format_spec_v10_1.pdf (Annex E.4.2.1 - AUDIODATA)
    pub enum SoundRate(u8) {
        /// 5.5 KHz
        Hz5500 = 0,
        /// 11 KHz
        Hz11000 = 1,
        /// 22 KHz
        Hz22000 = 2,
        /// 44 KHz
        Hz44000 = 3,
    }
}

nutype_enum! {
    /// FLV Sound Size
    ///
    /// Denotes the size of each sample in the audio data.
    ///
    /// Defined by:
    /// - video_file_format_spec_v10.pdf (Chapter 1 - The FLV File Format - Audio tags)
    /// - video_file_format_spec_v10_1.pdf (Annex E.4.2.1 - AUDIODATA)
    pub enum SoundSize(u8) {
        /// 8 bit
        Bit8 = 0,
        /// 16 bit
        Bit16 = 1,
    }
}

nutype_enum! {
    /// FLV Sound Type
    ///
    /// Denotes the number of channels in the audio data.
    ///
    /// Defined by:
    /// - video_file_format_spec_v10.pdf (Chapter 1 - The FLV File Format - Audio tags)
    /// - video_file_format_spec_v10_1.pdf (Annex E.4.2.1 - AUDIODATA)
    pub enum SoundType(u8) {
        /// Mono
        Mono = 0,
        /// Stereo
        Stereo = 1,
    }
}

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn test_sound_format() {
        let cases = [
            (
                0x00,
                SoundFormat::LinearPcmPlatformEndian,
                "SoundFormat::LinearPcmPlatformEndian",
            ),
            (0x01, SoundFormat::Adpcm, "SoundFormat::Adpcm"),
            (0x02, SoundFormat::Mp3, "SoundFormat::Mp3"),
            (0x03, SoundFormat::LinearPcmLittleEndian, "SoundFormat::LinearPcmLittleEndian"),
            (0x04, SoundFormat::Nellymoser16KhzMono, "SoundFormat::Nellymoser16KhzMono"),
            (0x05, SoundFormat::Nellymoser8KhzMono, "SoundFormat::Nellymoser8KhzMono"),
            (0x06, SoundFormat::Nellymoser, "SoundFormat::Nellymoser"),
            (0x07, SoundFormat::G711ALaw, "SoundFormat::G711ALaw"),
            (0x08, SoundFormat::G711MuLaw, "SoundFormat::G711MuLaw"),
            (0x0A, SoundFormat::Aac, "SoundFormat::Aac"),
            (0x0B, SoundFormat::Speex, "SoundFormat::Speex"),
            (0x0E, SoundFormat::Mp38Khz, "SoundFormat::Mp38Khz"),
            (0x0F, SoundFormat::DeviceSpecificSound, "SoundFormat::DeviceSpecificSound"),
        ];

        for (value, expected, name) in cases {
            let sound_format = SoundFormat::from(value);
            assert_eq!(sound_format, expected);
            assert_eq!(format!("{:?}", sound_format), name);
        }
    }

    #[test]
    fn test_sound_rate() {
        let cases = [
            (0x00, SoundRate::Hz5500, "SoundRate::Hz5500"),
            (0x01, SoundRate::Hz11000, "SoundRate::Hz11000"),
            (0x02, SoundRate::Hz22000, "SoundRate::Hz22000"),
            (0x03, SoundRate::Hz44000, "SoundRate::Hz44000"),
        ];

        for (value, expected, name) in cases {
            let sound_rate = SoundRate::from(value);
            assert_eq!(sound_rate, expected);
            assert_eq!(format!("{:?}", sound_rate), name);
        }
    }

    #[test]
    fn test_sound_size() {
        let cases = [
            (0x00, SoundSize::Bit8, "SoundSize::Bit8"),
            (0x01, SoundSize::Bit16, "SoundSize::Bit16"),
        ];

        for (value, expected, name) in cases {
            let sound_size = SoundSize::from(value);
            assert_eq!(sound_size, expected);
            assert_eq!(format!("{:?}", sound_size), name);
        }
    }

    #[test]
    fn test_sound_type() {
        let cases = [
            (0x00, SoundType::Mono, "SoundType::Mono"),
            (0x01, SoundType::Stereo, "SoundType::Stereo"),
        ];

        for (value, expected, name) in cases {
            let sound_type = SoundType::from(value);
            assert_eq!(sound_type, expected);
            assert_eq!(format!("{:?}", sound_type), name);
        }
    }

    #[test]
    fn test_audio_data_demux() {
        let mut reader = io::Cursor::new(Bytes::from(vec![0b10101101, 0b00000000, 1, 2, 3]));

        let audio_data = AudioData::demux(&mut reader).unwrap();
        assert_eq!(audio_data.sound_rate, SoundRate::Hz44000);
        assert_eq!(audio_data.sound_size, SoundSize::Bit8);
        assert_eq!(audio_data.sound_type, SoundType::Stereo);
        assert_eq!(
            audio_data.body,
            AudioDataBody::Aac(AacPacket::SequenceHeader(Bytes::from(vec![1, 2, 3])))
        );

        let mut reader = io::Cursor::new(Bytes::from(vec![0b10101101, 0b00100000, 1, 2, 3]));

        let audio_data = AudioData::demux(&mut reader).unwrap();
        assert_eq!(audio_data.sound_rate, SoundRate::Hz44000);
        assert_eq!(audio_data.sound_size, SoundSize::Bit8);
        assert_eq!(audio_data.sound_type, SoundType::Stereo);
        assert_eq!(
            audio_data.body,
            AudioDataBody::Aac(AacPacket::Unknown {
                aac_packet_type: AacPacketType(0b00100000),
                data: Bytes::from(vec![1, 2, 3])
            })
        );

        let mut reader = io::Cursor::new(Bytes::from(vec![0b10001101, 0b00000000, 1, 2, 3]));

        let audio_data = AudioData::demux(&mut reader).unwrap();
        assert_eq!(
            audio_data.body,
            AudioDataBody::Unknown {
                sound_format: SoundFormat(8),
                data: Bytes::from(vec![0, 1, 2, 3])
            }
        );
    }
}
