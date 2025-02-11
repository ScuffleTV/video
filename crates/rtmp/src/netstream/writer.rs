use std::io;

use bytes::Bytes;
use scuffle_amf0::{Amf0Encoder, Amf0Value};

use super::errors::NetStreamError;
use crate::chunk::{Chunk, ChunkEncoder, DefinedChunkStreamID};
use crate::messages::MessageTypeID;

pub struct NetStreamWriter {}

impl NetStreamWriter {
    fn write_chunk(encoder: &ChunkEncoder, amf0_writer: Bytes, writer: &mut impl io::Write) -> Result<(), NetStreamError> {
        encoder.write_chunk(
            writer,
            Chunk::new(
                DefinedChunkStreamID::Command as u32,
                0,
                MessageTypeID::CommandAMF0,
                0,
                amf0_writer,
            ),
        )?;

        Ok(())
    }

    pub fn write_on_status(
        encoder: &ChunkEncoder,
        writer: &mut impl io::Write,
        transaction_id: f64,
        level: &str,
        code: &str,
        description: &str,
    ) -> Result<(), NetStreamError> {
        let mut amf0_writer = Vec::new();

        Amf0Encoder::encode_string(&mut amf0_writer, "onStatus")?;
        Amf0Encoder::encode_number(&mut amf0_writer, transaction_id)?;
        Amf0Encoder::encode_null(&mut amf0_writer)?;
        Amf0Encoder::encode_object(
            &mut amf0_writer,
            &[
                ("level".into(), Amf0Value::String(level.into())),
                ("code".into(), Amf0Value::String(code.into())),
                ("description".into(), Amf0Value::String(description.into())),
            ],
        )?;

        Self::write_chunk(encoder, Bytes::from(amf0_writer), writer)
    }
}

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use bytes::{BufMut, BytesMut};
    use scuffle_amf0::Amf0Decoder;

    use super::*;
    use crate::chunk::ChunkDecoder;

    #[test]
    fn test_netstream_write_on_status() {
        let encoder = ChunkEncoder::default();
        let mut buf = BytesMut::new();

        NetStreamWriter::write_on_status(&encoder, &mut (&mut buf).writer(), 1.0, "status", "idk", "description").unwrap();

        let mut decoder = ChunkDecoder::default();

        let chunk = decoder.read_chunk(&mut buf).expect("read chunk").expect("chunk");
        assert_eq!(chunk.basic_header.chunk_stream_id, 0x03);
        assert_eq!(chunk.message_header.msg_type_id as u8, 0x14);
        assert_eq!(chunk.message_header.msg_stream_id, 0);

        let mut amf0_reader = Amf0Decoder::new(&chunk.payload);
        let values = amf0_reader.decode_all().unwrap();

        assert_eq!(values.len(), 4);
        assert_eq!(values[0], Amf0Value::String("onStatus".into())); // command name
        assert_eq!(values[1], Amf0Value::Number(1.0)); // transaction id
        assert_eq!(values[2], Amf0Value::Null); // command object
        assert_eq!(
            values[3],
            Amf0Value::Object(
                vec![
                    ("level".into(), Amf0Value::String("status".into())),
                    ("code".into(), Amf0Value::String("idk".into())),
                    ("description".into(), Amf0Value::String("description".into())),
                ]
                .into()
            )
        ); // info object
    }
}
