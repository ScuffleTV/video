use scuffle_amf0::{Amf0Decoder, Amf0Marker};

use super::define::{MessageTypeID, RtmpMessageData};
use super::errors::MessageError;
use crate::chunk::Chunk;
use crate::protocol_control_messages::ProtocolControlMessageReader;

pub struct MessageParser;

impl MessageParser {
    pub fn parse(chunk: &Chunk) -> Result<Option<RtmpMessageData<'_>>, MessageError> {
        match chunk.message_header.msg_type_id {
            // Protocol Control Messages
            MessageTypeID::CommandAMF0 => {
                let mut amf_reader = Amf0Decoder::new(&chunk.payload);
                let command_name = amf_reader.decode_with_type(Amf0Marker::String)?;
                let transaction_id = amf_reader.decode_with_type(Amf0Marker::Number)?;
                let command_object = match amf_reader.decode_with_type(Amf0Marker::Object) {
                    Ok(val) => val,
                    Err(_) => amf_reader.decode_with_type(Amf0Marker::Null)?,
                };

                let others = amf_reader.decode_all()?;

                Ok(Some(RtmpMessageData::Amf0Command {
                    command_name,
                    transaction_id,
                    command_object,
                    others,
                }))
            }
            // Data Messages - AUDIO
            MessageTypeID::Audio => Ok(Some(RtmpMessageData::AudioData {
                data: chunk.payload.clone(),
            })),
            // Data Messages - VIDEO
            MessageTypeID::Video => Ok(Some(RtmpMessageData::VideoData {
                data: chunk.payload.clone(),
            })),
            // Protocol Control Messages
            MessageTypeID::SetChunkSize => {
                let chunk_size = ProtocolControlMessageReader::read_set_chunk_size(&chunk.payload)?;

                Ok(Some(RtmpMessageData::SetChunkSize { chunk_size }))
            }
            // Metadata
            MessageTypeID::DataAMF0 => Ok(Some(RtmpMessageData::Amf0Data {
                data: chunk.payload.clone(),
            })),
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use std::borrow::Cow;

    use bytes::Bytes;
    use scuffle_amf0::{Amf0Encoder, Amf0Value};

    use super::*;

    #[test]
    fn test_parse_command() {
        let mut amf0_writer = Vec::new();

        Amf0Encoder::encode_string(&mut amf0_writer, "connect").unwrap();
        Amf0Encoder::encode_number(&mut amf0_writer, 1.0).unwrap();
        Amf0Encoder::encode_null(&mut amf0_writer).unwrap();

        let amf_data = Bytes::from(amf0_writer);

        let chunk = Chunk::new(0, 0, MessageTypeID::CommandAMF0, 0, amf_data);

        let message = MessageParser::parse(&chunk).expect("no errors").expect("message");
        match message {
            RtmpMessageData::Amf0Command {
                command_name,
                transaction_id,
                command_object,
                others,
            } => {
                assert_eq!(command_name, Amf0Value::String(Cow::Borrowed("connect")));
                assert_eq!(transaction_id, Amf0Value::Number(1.0));
                assert_eq!(command_object, Amf0Value::Null);
                assert_eq!(others, vec![]);
            }
            _ => unreachable!("wrong message type"),
        }
    }

    #[test]
    fn test_parse_audio_packet() {
        let chunk = Chunk::new(0, 0, MessageTypeID::Audio, 0, vec![0x00, 0x00, 0x00, 0x00].into());

        let message = MessageParser::parse(&chunk).expect("no errors").expect("message");
        match message {
            RtmpMessageData::AudioData { data } => {
                assert_eq!(data, vec![0x00, 0x00, 0x00, 0x00]);
            }
            _ => unreachable!("wrong message type"),
        }
    }

    #[test]
    fn test_parse_video_packet() {
        let chunk = Chunk::new(0, 0, MessageTypeID::Video, 0, vec![0x00, 0x00, 0x00, 0x00].into());

        let message = MessageParser::parse(&chunk).expect("no errors").expect("message");
        match message {
            RtmpMessageData::VideoData { data } => {
                assert_eq!(data, vec![0x00, 0x00, 0x00, 0x00]);
            }
            _ => unreachable!("wrong message type"),
        }
    }

    #[test]
    fn test_parse_set_chunk_size() {
        let chunk = Chunk::new(0, 0, MessageTypeID::SetChunkSize, 0, vec![0x00, 0xFF, 0xFF, 0xFF].into());

        let message = MessageParser::parse(&chunk).expect("no errors").expect("message");
        match message {
            RtmpMessageData::SetChunkSize { chunk_size } => {
                assert_eq!(chunk_size, 0x00FFFFFF);
            }
            _ => unreachable!("wrong message type"),
        }
    }

    #[test]
    fn test_parse_metadata() {
        let mut amf0_writer = Vec::new();

        Amf0Encoder::encode_string(&mut amf0_writer, "onMetaData").unwrap();
        Amf0Encoder::encode_object(&mut amf0_writer, &[("duration".into(), Amf0Value::Number(0.0))]).unwrap();

        let amf_data = Bytes::from(amf0_writer);
        let chunk = Chunk::new(0, 0, MessageTypeID::DataAMF0, 0, amf_data.clone());

        let message = MessageParser::parse(&chunk).expect("no errors").expect("message");
        match message {
            RtmpMessageData::Amf0Data { data } => {
                assert_eq!(data, amf_data);
            }
            _ => unreachable!("wrong message type"),
        }
    }

    #[test]
    fn test_unsupported_message_type() {
        let chunk = Chunk::new(0, 0, MessageTypeID::Aggregate, 0, vec![0x00, 0x00, 0x00, 0x00].into());

        assert!(MessageParser::parse(&chunk).expect("no errors").is_none())
    }
}
