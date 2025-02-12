use std::io;

use byteorder::{BigEndian, WriteBytesExt};
use bytes::Bytes;

use super::errors::ProtocolControlMessageError;
use crate::chunk::{Chunk, ChunkEncoder};
use crate::messages::MessageTypeID;

pub struct ProtocolControlMessagesWriter;

impl ProtocolControlMessagesWriter {
    pub fn write_set_chunk_size(
        encoder: &ChunkEncoder,
        writer: &mut impl io::Write,
        chunk_size: u32, // 31 bits
    ) -> Result<(), ProtocolControlMessageError> {
        // According to spec the first bit must be 0.
        let chunk_size = chunk_size & 0x7FFFFFFF; // 31 bits only

        encoder.write_chunk(
            writer,
            Chunk::new(
                2, // chunk stream must be 2
                0, // timestamps are ignored
                MessageTypeID::SetChunkSize,
                0, // message stream id is ignored
                Bytes::from(chunk_size.to_be_bytes().to_vec()),
            ),
        )?;

        Ok(())
    }

    pub fn write_window_acknowledgement_size(
        encoder: &ChunkEncoder,
        writer: &mut impl io::Write,
        window_size: u32,
    ) -> Result<(), ProtocolControlMessageError> {
        encoder.write_chunk(
            writer,
            Chunk::new(
                2, // chunk stream must be 2
                0, // timestamps are ignored
                MessageTypeID::WindowAcknowledgementSize,
                0, // message stream id is ignored
                Bytes::from(window_size.to_be_bytes().to_vec()),
            ),
        )?;

        Ok(())
    }

    pub fn write_set_peer_bandwidth(
        encoder: &ChunkEncoder,
        writer: &mut impl io::Write,
        window_size: u32,
        limit_type: u8,
    ) -> Result<(), ProtocolControlMessageError> {
        let mut data = Vec::new();
        data.write_u32::<BigEndian>(window_size).expect("Failed to write window size");
        data.write_u8(limit_type).expect("Failed to write limit type");

        encoder.write_chunk(
            writer,
            Chunk::new(
                2, // chunk stream must be 2
                0, // timestamps are ignored
                MessageTypeID::SetPeerBandwidth,
                0, // message stream id is ignored
                Bytes::from(data),
            ),
        )?;

        Ok(())
    }
}

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use bytes::{BufMut, BytesMut};

    use super::*;
    use crate::chunk::ChunkDecoder;

    #[test]
    fn test_writer_write_set_chunk_size() {
        let encoder = ChunkEncoder::default();
        let mut buf = BytesMut::new();

        ProtocolControlMessagesWriter::write_set_chunk_size(&encoder, &mut (&mut buf).writer(), 1).unwrap();

        let mut decoder = ChunkDecoder::default();

        let chunk = decoder.read_chunk(&mut buf).expect("read chunk").expect("chunk");
        assert_eq!(chunk.basic_header.chunk_stream_id, 0x02);
        assert_eq!(chunk.message_header.msg_type_id as u8, 0x01);
        assert_eq!(chunk.message_header.msg_stream_id, 0);
        assert_eq!(chunk.payload, vec![0x00, 0x00, 0x00, 0x01]);
    }

    #[test]
    fn test_writer_window_acknowledgement_size() {
        let encoder = ChunkEncoder::default();
        let mut buf = BytesMut::new();

        ProtocolControlMessagesWriter::write_window_acknowledgement_size(&encoder, &mut (&mut buf).writer(), 1).unwrap();

        let mut decoder = ChunkDecoder::default();

        let chunk = decoder.read_chunk(&mut buf).expect("read chunk").expect("chunk");
        assert_eq!(chunk.basic_header.chunk_stream_id, 0x02);
        assert_eq!(chunk.message_header.msg_type_id as u8, 0x05);
        assert_eq!(chunk.message_header.msg_stream_id, 0);
        assert_eq!(chunk.payload, vec![0x00, 0x00, 0x00, 0x01]);
    }

    #[test]
    fn test_writer_set_peer_bandwidth() {
        let encoder = ChunkEncoder::default();
        let mut buf = BytesMut::new();

        ProtocolControlMessagesWriter::write_set_peer_bandwidth(&encoder, &mut (&mut buf).writer(), 1, 2).unwrap();

        let mut decoder = ChunkDecoder::default();

        let chunk = decoder.read_chunk(&mut buf).expect("read chunk").expect("chunk");
        assert_eq!(chunk.basic_header.chunk_stream_id, 0x02);
        assert_eq!(chunk.message_header.msg_type_id as u8, 0x06);
        assert_eq!(chunk.message_header.msg_stream_id, 0);
        assert_eq!(chunk.payload, vec![0x00, 0x00, 0x00, 0x01, 0x02]);
    }
}
