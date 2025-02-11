use std::io;

use byteorder::{BigEndian, LittleEndian, WriteBytesExt};

use super::define::{Chunk, ChunkMessageHeader, ChunkType, INIT_CHUNK_SIZE};
use super::errors::ChunkEncodeError;

/// A chunk encoder.
///
/// This is used to encode chunks into a stream.
pub struct ChunkEncoder {
    chunk_size: usize,
}

impl Default for ChunkEncoder {
    fn default() -> Self {
        Self {
            chunk_size: INIT_CHUNK_SIZE,
        }
    }
}

impl ChunkEncoder {
    /// Set the chunk size.
    pub fn set_chunk_size(&mut self, chunk_size: usize) {
        self.chunk_size = chunk_size;
    }

    /// Internal function to write the basic header.
    #[inline]
    fn write_basic_header(writer: &mut impl io::Write, fmt: ChunkType, csid: u32) -> Result<(), ChunkEncodeError> {
        let fmt = fmt as u8;

        if csid >= 64 + 255 {
            writer.write_u8((fmt << 6) | 1)?;
            let csid = csid - 64;

            let div = csid / 256;
            let rem = csid % 256;

            writer.write_u8(rem as u8)?;
            writer.write_u8(div as u8)?;
        } else if csid >= 64 {
            writer.write_u8(fmt << 6)?;
            writer.write_u8((csid - 64) as u8)?;
        } else {
            writer.write_u8((fmt << 6) | csid as u8)?;
        }

        Ok(())
    }

    /// Internal function to write the message header.
    #[inline]
    fn write_message_header(
        writer: &mut impl io::Write,
        message_header: &ChunkMessageHeader,
    ) -> Result<(), ChunkEncodeError> {
        let timestamp = if message_header.timestamp >= 0xFFFFFF {
            0xFFFFFF
        } else {
            message_header.timestamp
        };

        writer.write_u24::<BigEndian>(timestamp)?;
        writer.write_u24::<BigEndian>(message_header.msg_length)?;
        writer.write_u8(message_header.msg_type_id as u8)?;
        writer.write_u32::<LittleEndian>(message_header.msg_stream_id)?;

        if message_header.is_extended_timestamp() {
            Self::write_extened_timestamp(writer, message_header.timestamp)?;
        }

        Ok(())
    }

    /// Internal function to write the extended timestamp.
    #[inline]
    fn write_extened_timestamp(writer: &mut impl io::Write, timestamp: u32) -> Result<(), ChunkEncodeError> {
        writer.write_u32::<BigEndian>(timestamp)?;

        Ok(())
    }

    /// Write a chunk into some writer.
    pub fn write_chunk(&self, writer: &mut impl io::Write, mut chunk_info: Chunk) -> Result<(), ChunkEncodeError> {
        Self::write_basic_header(writer, ChunkType::Type0, chunk_info.basic_header.chunk_stream_id)?;

        Self::write_message_header(writer, &chunk_info.message_header)?;

        while !chunk_info.payload.is_empty() {
            let cur_payload_size = if chunk_info.payload.len() > self.chunk_size {
                self.chunk_size
            } else {
                chunk_info.payload.len()
            };

            let payload_bytes = chunk_info.payload.split_to(cur_payload_size);
            writer.write_all(&payload_bytes[..])?;

            if !chunk_info.payload.is_empty() {
                Self::write_basic_header(writer, ChunkType::Type3, chunk_info.basic_header.chunk_stream_id)?;

                if chunk_info.message_header.is_extended_timestamp() {
                    Self::write_extened_timestamp(writer, chunk_info.message_header.timestamp)?;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use std::io;

    use bytes::Bytes;

    use super::*;
    use crate::messages::MessageTypeID;

    #[test]
    fn test_encoder_error_display() {
        let error = ChunkEncodeError::UnknownReadState;
        assert_eq!(format!("{}", error), "unknown read state");

        let error = ChunkEncodeError::Io(io::Error::from(io::ErrorKind::Other));
        assert_eq!(format!("{}", error), "io error: other error");
    }

    #[test]
    fn test_encoder_write_small_chunk() {
        let encoder = ChunkEncoder::default();
        let mut writer = Vec::new();

        let chunk = Chunk::new(
            0,
            0,
            MessageTypeID::Abort,
            0,
            Bytes::from(vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07]),
        );

        encoder.write_chunk(&mut writer, chunk).unwrap();

        #[rustfmt::skip]
        assert_eq!(
            writer,
            vec![
                (0x00 << 6), // chunk basic header - fmt: 0, csid: 0
                0x00, 0x00, 0x00, // timestamp (0)
                0x00, 0x00, 0x08, // message length (8 bytes)
                0x02, // message type id (abort)
                0x00, 0x00, 0x00, 0x00, // message stream id (0)
                0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, // message payload
            ]
        );
    }

    #[test]
    fn test_encoder_write_large_chunk() {
        let encoder = ChunkEncoder::default();
        let mut writer = Vec::new();

        let mut payload = Vec::new();
        for i in 0..129 {
            payload.push(i);
        }

        let chunk = Chunk::new(10, 100, MessageTypeID::Audio, 13, Bytes::from(payload));

        encoder.write_chunk(&mut writer, chunk).unwrap();

        #[rustfmt::skip]
        let mut expected = vec![
            0x0A, // chunk basic header - fmt: 0, csid: 10 (the format should have been fixed to 0)
            0x00, 0x00, 0x64, // timestamp (100)
            0x00, 0x00, 0x81, // message length (129 bytes)
            0x08, // message type id (audio)
            0x0D, 0x00, 0x00, 0x00, // message stream id (13)
        ];

        for i in 0..128 {
            expected.push(i);
        }

        expected.push((0x03 << 6) | 0x0A); // chunk basic header - fmt: 3, csid: 10
        expected.push(128); // The rest of the payload should have been written

        assert_eq!(writer, expected);
    }

    #[test]
    fn test_encoder_extended_timestamp() {
        let encoder = ChunkEncoder::default();
        let mut writer = Vec::new();

        let chunk = Chunk::new(
            0,
            0xFFFFFFFF,
            MessageTypeID::Abort,
            0,
            Bytes::from(vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07]),
        );

        encoder.write_chunk(&mut writer, chunk).unwrap();

        #[rustfmt::skip]
        assert_eq!(
            writer,
            vec![
                (0x00 << 6), // chunk basic header - fmt: 0, csid: 0
                0xFF, 0xFF, 0xFF, // timestamp (0xFFFFFF)
                0x00, 0x00, 0x08, // message length (8 bytes)
                0x02, // message type id (abort)
                0x00, 0x00, 0x00,
                0x00, // message stream id (0)
                0xFF, 0xFF, 0xFF,
                0xFF, // extended timestamp (1)
                0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, // message payload
            ]
        );
    }

    #[test]
    fn test_encoder_extended_timestamp_ext() {
        let encoder = ChunkEncoder::default();
        let mut writer = Vec::new();

        let mut payload = Vec::new();
        for i in 0..129 {
            payload.push(i);
        }

        let chunk = Chunk::new(0, 0xFFFFFFFF, MessageTypeID::Abort, 0, Bytes::from(payload));

        encoder.write_chunk(&mut writer, chunk).unwrap();

        #[rustfmt::skip]
        let mut expected = vec![
            (0x00 << 6), // chunk basic header - fmt: 0, csid: 0
            0xFF, 0xFF, 0xFF, // timestamp (0xFFFFFF)
            0x00, 0x00, 0x81, // message length (8 bytes)
            0x02, // message type id (abort)
            0x00, 0x00, 0x00, 0x00, // message stream id (0)
            0xFF, 0xFF, 0xFF, 0xFF, // extended timestamp (1)
        ];

        for i in 0..128 {
            expected.push(i);
        }

        expected.push(0x03 << 6); // chunk basic header - fmt: 3, csid: 0
        expected.extend(vec![0xFF, 0xFF, 0xFF, 0xFF]); // extended timestamp
        expected.push(128); // The rest of the payload should have been written

        assert_eq!(writer, expected);
    }

    #[test]
    fn test_encoder_extended_csid() {
        let encoder = ChunkEncoder::default();
        let mut writer = Vec::new();

        let chunk = Chunk::new(
            64,
            0,
            MessageTypeID::Abort,
            0,
            Bytes::from(vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07]),
        );

        encoder.write_chunk(&mut writer, chunk).unwrap();

        #[rustfmt::skip]
        assert_eq!(
            writer,
            vec![
                (0x00 << 6), // chunk basic header - fmt: 0, csid: 0
                0x00, // extended csid (64 + 0) = 64
                0x00, 0x00, 0x00, // timestamp (0)
                0x00, 0x00, 0x08, // message length (8 bytes)
                0x02, // message type id (abort)
                0x00, 0x00, 0x00, 0x00, // message stream id (0)
                0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, // message payload
            ]
        );
    }

    #[test]
    fn test_encoder_extended_csid_ext() {
        let encoder = ChunkEncoder::default();
        let mut writer = Vec::new();

        let chunk = Chunk::new(
            320,
            0,
            MessageTypeID::Abort,
            0,
            Bytes::from(vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07]),
        );

        encoder.write_chunk(&mut writer, chunk).unwrap();

        #[rustfmt::skip]
        assert_eq!(
            writer,
            vec![
                0x01, // chunk basic header - fmt: 0, csid: 1
                0x00, // extended csid (64 + 0) = 64
                0x01, // extended csid (256 * 1) = 256 + 64 + 0 = 320
                0x00, 0x00, 0x00, // timestamp (0)
                0x00, 0x00, 0x08, // message length (8 bytes)
                0x02, // message type id (abort)
                0x00, 0x00, 0x00, 0x00, // message stream id (0)
                0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, // message payload
            ]
        );
    }
}
