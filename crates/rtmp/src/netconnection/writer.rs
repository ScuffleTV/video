use std::io;

use bytes::Bytes;
use scuffle_amf0::{Amf0Encoder, Amf0Value};

use super::errors::NetConnectionError;
use crate::chunk::{Chunk, ChunkEncoder, DefinedChunkStreamID};
use crate::messages::MessageTypeID;

pub struct NetConnection;

impl NetConnection {
    fn write_chunk(encoder: &ChunkEncoder, amf0: Bytes, writer: &mut impl io::Write) -> Result<(), NetConnectionError> {
        encoder.write_chunk(
            writer,
            Chunk::new(DefinedChunkStreamID::Command as u32, 0, MessageTypeID::CommandAMF0, 0, amf0),
        )?;

        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn write_connect_response(
        encoder: &ChunkEncoder,
        writer: &mut impl io::Write,
        transaction_id: f64,
        fmsver: &str,
        capabilities: f64,
        code: &str,
        level: &str,
        description: &str,
        encoding: f64,
    ) -> Result<(), NetConnectionError> {
        let mut amf0_writer = Vec::new();

        Amf0Encoder::encode_string(&mut amf0_writer, "_result")?;
        Amf0Encoder::encode_number(&mut amf0_writer, transaction_id)?;
        Amf0Encoder::encode_object(
            &mut amf0_writer,
            &[
                ("fmsVer".into(), Amf0Value::String(fmsver.into())),
                ("capabilities".into(), Amf0Value::Number(capabilities)),
            ],
        )?;
        Amf0Encoder::encode_object(
            &mut amf0_writer,
            &[
                ("level".into(), Amf0Value::String(level.into())),
                ("code".into(), Amf0Value::String(code.into())),
                ("description".into(), Amf0Value::String(description.into())),
                ("objectEncoding".into(), Amf0Value::Number(encoding)),
            ],
        )?;

        Self::write_chunk(encoder, Bytes::from(amf0_writer), writer)
    }

    pub fn write_create_stream_response(
        encoder: &ChunkEncoder,
        writer: &mut impl io::Write,
        transaction_id: f64,
        stream_id: f64,
    ) -> Result<(), NetConnectionError> {
        let mut amf0_writer = Vec::new();

        Amf0Encoder::encode_string(&mut amf0_writer, "_result")?;
        Amf0Encoder::encode_number(&mut amf0_writer, transaction_id)?;
        Amf0Encoder::encode_null(&mut amf0_writer)?;
        Amf0Encoder::encode_number(&mut amf0_writer, stream_id)?;

        Self::write_chunk(encoder, Bytes::from(amf0_writer), writer)
    }
}

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use std::borrow::Cow;

    use bytes::{BufMut, BytesMut};
    use scuffle_amf0::Amf0Decoder;

    use super::*;
    use crate::chunk::ChunkDecoder;

    #[test]
    fn test_netconnection_connect_response() {
        let encoder = ChunkEncoder::default();
        let mut buf = BytesMut::new();

        NetConnection::write_connect_response(
            &encoder,
            &mut (&mut buf).writer(),
            1.0,
            "flashver",
            31.0,
            "status",
            "idk",
            "description",
            0.0,
        )
        .unwrap();

        let mut decoder = ChunkDecoder::default();

        let chunk = decoder.read_chunk(&mut buf).expect("read chunk").expect("chunk");
        assert_eq!(chunk.basic_header.chunk_stream_id, 0x03);
        assert_eq!(chunk.message_header.msg_type_id as u8, 0x14);
        assert_eq!(chunk.message_header.msg_stream_id, 0);

        let mut amf0_reader = Amf0Decoder::new(&chunk.payload);
        let values = amf0_reader.decode_all().unwrap();

        assert_eq!(values.len(), 4);
        assert_eq!(values[0], Amf0Value::String("_result".into())); // command name
        assert_eq!(values[1], Amf0Value::Number(1.0)); // transaction id
        assert_eq!(
            values[2],
            Amf0Value::Object(Cow::Owned(vec![
                ("fmsVer".into(), Amf0Value::String("flashver".into())),
                ("capabilities".into(), Amf0Value::Number(31.0)),
            ]))
        ); // command object
        assert_eq!(
            values[3],
            Amf0Value::Object(Cow::Owned(vec![
                ("level".into(), Amf0Value::String("idk".into())),
                ("code".into(), Amf0Value::String("status".into())),
                ("description".into(), Amf0Value::String("description".into())),
                ("objectEncoding".into(), Amf0Value::Number(0.0)),
            ]))
        ); // info object
    }

    #[test]
    fn test_netconnection_create_stream_response() {
        let encoder = ChunkEncoder::default();
        let mut buf = BytesMut::new();

        NetConnection::write_create_stream_response(&encoder, &mut (&mut buf).writer(), 1.0, 1.0).unwrap();

        let mut decoder = ChunkDecoder::default();

        let chunk = decoder.read_chunk(&mut buf).expect("read chunk").expect("chunk");
        assert_eq!(chunk.basic_header.chunk_stream_id, 0x03);
        assert_eq!(chunk.message_header.msg_type_id as u8, 0x14);
        assert_eq!(chunk.message_header.msg_stream_id, 0);

        let mut amf0_reader = Amf0Decoder::new(&chunk.payload);
        let values = amf0_reader.decode_all().unwrap();

        assert_eq!(values.len(), 4);
        assert_eq!(values[0], Amf0Value::String("_result".into())); // command name
        assert_eq!(values[1], Amf0Value::Number(1.0)); // transaction id
        assert_eq!(values[2], Amf0Value::Null); // command object
        assert_eq!(values[3], Amf0Value::Number(1.0)); // stream id
    }
}
