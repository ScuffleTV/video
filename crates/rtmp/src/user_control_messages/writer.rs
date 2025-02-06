use std::io;

use byteorder::{BigEndian, WriteBytesExt};

use super::define;
use super::errors::EventMessagesError;
use crate::chunk::{Chunk, ChunkEncoder};
use crate::messages::MessageTypeID;

pub struct EventMessagesWriter;

impl EventMessagesWriter {
    pub fn write_stream_begin(
        encoder: &ChunkEncoder,
        writer: &mut impl io::Write,
        stream_id: u32,
    ) -> Result<(), EventMessagesError> {
        let mut data = Vec::new();

        data.write_u16::<BigEndian>(define::RTMP_EVENT_STREAM_BEGIN)
            .expect("write u16");
        data.write_u32::<BigEndian>(stream_id).expect("write u32");

        encoder.write_chunk(writer, Chunk::new(0x02, 0, MessageTypeID::UserControlEvent, 0, data.into()))?;

        Ok(())
    }
}

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use bytes::{BufMut, Bytes, BytesMut};

    use super::*;
    use crate::chunk::ChunkDecoder;

    #[test]
    fn test_write_stream_begin() {
        let mut buf = BytesMut::new();
        let encoder = ChunkEncoder::default();

        EventMessagesWriter::write_stream_begin(&encoder, &mut (&mut buf).writer(), 1).unwrap();

        let mut decoder = ChunkDecoder::default();

        let chunk = decoder.read_chunk(&mut buf).expect("read chunk").expect("chunk");
        assert_eq!(chunk.basic_header.chunk_stream_id, 0x02);
        assert_eq!(chunk.message_header.msg_type_id as u8, 0x04);
        assert_eq!(chunk.message_header.msg_stream_id, 0);
        assert_eq!(chunk.payload, Bytes::from(vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x01]));
    }
}
