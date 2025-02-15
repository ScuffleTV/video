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
            MessageTypeID::DataAMF0 | MessageTypeID::DataAMF3 => Ok(Some(RtmpMessageData::AmfData {
                data: chunk.payload.clone(),
            })),
            _ => Ok(None),
        }
    }
}
