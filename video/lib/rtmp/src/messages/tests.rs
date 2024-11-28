use std::collections::HashMap;

use amf0::{Amf0ReadError, Amf0Value, Amf0Writer};
use bytesio::bytes_writer::BytesWriter;

use super::{MessageError, MessageParser, MessageTypeID, RtmpMessageData};
use crate::chunk::{Chunk, ChunkEncodeError};
use crate::protocol_control_messages::ProtocolControlMessageError;

#[test]
fn test_error_display() {
	let error = MessageError::Amf0Read(Amf0ReadError::WrongType);
	assert_eq!(error.to_string(), "amf0 read error: wrong type");

	let error =
		MessageError::ProtocolControlMessage(ProtocolControlMessageError::ChunkEncode(ChunkEncodeError::UnknownReadState));
	assert_eq!(
		error.to_string(),
		"protocol control message error: chunk encode error: unknown read state"
	);
}

#[test]
fn test_parse_command() {
	let mut amf0_writer = BytesWriter::default();

	Amf0Writer::write_string(&mut amf0_writer, "connect").unwrap();
	Amf0Writer::write_number(&mut amf0_writer, 1.0).unwrap();
	Amf0Writer::write_null(&mut amf0_writer).unwrap();

	let chunk = Chunk::new(0, 0, MessageTypeID::CommandAMF0, 0, amf0_writer.dispose());

	let message = MessageParser::parse(chunk).expect("no errors").expect("message");
	match message {
		RtmpMessageData::Amf0Command {
			command_name,
			transaction_id,
			command_object,
			others,
		} => {
			assert_eq!(command_name, Amf0Value::String("connect".to_string()));
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

	let message = MessageParser::parse(chunk).expect("no errors").expect("message");
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

	let message = MessageParser::parse(chunk).expect("no errors").expect("message");
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

	let message = MessageParser::parse(chunk).expect("no errors").expect("message");
	match message {
		RtmpMessageData::SetChunkSize { chunk_size } => {
			assert_eq!(chunk_size, 0x00FFFFFF);
		}
		_ => unreachable!("wrong message type"),
	}
}

#[test]
fn test_parse_metadata() {
	let mut amf0_writer = BytesWriter::default();

	Amf0Writer::write_string(&mut amf0_writer, "onMetaData").unwrap();
	Amf0Writer::write_object(
		&mut amf0_writer,
		&HashMap::from([("duration".to_string(), Amf0Value::Number(0.0))]),
	)
	.unwrap();

	let amf_data = amf0_writer.dispose();

	let chunk = Chunk::new(0, 0, MessageTypeID::DataAMF0, 0, amf_data.clone());

	let message = MessageParser::parse(chunk).expect("no errors").expect("message");
	match message {
		RtmpMessageData::AmfData { data } => {
			assert_eq!(data, amf_data);
		}
		_ => unreachable!("wrong message type"),
	}
}

#[test]
fn test_unsupported_message_type() {
	let chunk = Chunk::new(0, 0, MessageTypeID::Aggregate, 0, vec![0x00, 0x00, 0x00, 0x00].into());

	assert!(MessageParser::parse(chunk).expect("no errors").is_none())
}
