use bytes::Bytes;
use bytesio::bytes_writer::BytesWriter;

use crate::chunk::{ChunkDecoder, ChunkEncodeError, ChunkEncoder};
use crate::user_control_messages::{EventMessagesError, EventMessagesWriter};

#[test]
fn test_error_display() {
	let error = EventMessagesError::ChunkEncode(ChunkEncodeError::UnknownReadState);
	assert_eq!(format!("{}", error), "chunk encode error: unknown read state");
}

#[test]
fn test_write_stream_begin() {
	let mut writer = BytesWriter::default();
	let encoder = ChunkEncoder::default();

	EventMessagesWriter::write_stream_begin(&encoder, &mut writer, 1).unwrap();

	let mut decoder = ChunkDecoder::default();
	decoder.extend_data(&writer.dispose());

	let chunk = decoder.read_chunk().unwrap().unwrap();
	assert_eq!(chunk.basic_header.chunk_stream_id, 0x02);
	assert_eq!(chunk.message_header.msg_type_id as u8, 0x04);
	assert_eq!(chunk.message_header.msg_stream_id, 0);
	assert_eq!(chunk.payload, Bytes::from(vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x01]));
}
