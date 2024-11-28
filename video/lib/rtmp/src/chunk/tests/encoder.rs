use std::io;

use bytes::Bytes;
use bytesio::bytes_writer::BytesWriter;

use crate::chunk::{Chunk, ChunkEncodeError, ChunkEncoder};
use crate::messages::MessageTypeID;

#[test]
fn test_encoder_error_display() {
	let error = ChunkEncodeError::UnknownReadState;
	assert_eq!(format!("{}", error), "unknown read state");

	let error = ChunkEncodeError::IO(io::Error::from(io::ErrorKind::Other));
	assert_eq!(format!("{}", error), "io error: other error");
}

#[test]
fn test_encoder_write_small_chunk() {
	let encoder = ChunkEncoder::default();
	let mut writer = BytesWriter::default();

	let chunk = Chunk::new(
		0,
		0,
		MessageTypeID::Abort,
		0,
		Bytes::from(vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07]),
	);

	encoder.write_chunk(&mut writer, chunk).unwrap();

	let result = writer.dispose();

	#[rustfmt::skip]
    assert_eq!(
        result,
        Bytes::from(vec![
            (0x00 << 6), // chunk basic header - fmt: 0, csid: 0
            0x00, 0x00, 0x00, // timestamp (0)
            0x00, 0x00, 0x08, // message length (8 bytes)
            0x02, // message type id (abort)
            0x00, 0x00, 0x00, 0x00, // message stream id (0)
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, // message payload
        ])
    );
}

#[test]
fn test_encoder_write_large_chunk() {
	let encoder = ChunkEncoder::default();
	let mut writer = BytesWriter::default();

	let mut payload = Vec::new();
	for i in 0..129 {
		payload.push(i);
	}

	let chunk = Chunk::new(10, 100, MessageTypeID::Audio, 13, Bytes::from(payload));

	encoder.write_chunk(&mut writer, chunk).unwrap();

	let result = writer.dispose();

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

	assert_eq!(result, Bytes::from(expected));
}

#[test]
fn test_encoder_extended_timestamp() {
	let encoder = ChunkEncoder::default();
	let mut writer = BytesWriter::default();

	let chunk = Chunk::new(
		0,
		0xFFFFFFFF,
		MessageTypeID::Abort,
		0,
		Bytes::from(vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07]),
	);

	encoder.write_chunk(&mut writer, chunk).unwrap();

	let result = writer.dispose();

	#[rustfmt::skip]
    assert_eq!(
        result,
        Bytes::from(vec![
            (0x00 << 6), // chunk basic header - fmt: 0, csid: 0
            0xFF, 0xFF, 0xFF, // timestamp (0xFFFFFF)
            0x00, 0x00, 0x08, // message length (8 bytes)
            0x02, // message type id (abort)
            0x00, 0x00, 0x00,
            0x00, // message stream id (0)
            0xFF, 0xFF, 0xFF,
            0xFF, // extended timestamp (1)
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, // message payload
        ])
    );
}

#[test]
fn test_encoder_extended_timestamp_ext() {
	let encoder = ChunkEncoder::default();
	let mut writer = BytesWriter::default();

	let mut payload = Vec::new();
	for i in 0..129 {
		payload.push(i);
	}

	let chunk = Chunk::new(0, 0xFFFFFFFF, MessageTypeID::Abort, 0, Bytes::from(payload));

	encoder.write_chunk(&mut writer, chunk).unwrap();

	let result = writer.dispose();

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

	assert_eq!(result, Bytes::from(expected));
}

#[test]
fn test_encoder_extended_csid() {
	let encoder = ChunkEncoder::default();
	let mut writer = BytesWriter::default();

	let chunk = Chunk::new(
		64,
		0,
		MessageTypeID::Abort,
		0,
		Bytes::from(vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07]),
	);

	encoder.write_chunk(&mut writer, chunk).unwrap();

	let result = writer.dispose();

	#[rustfmt::skip]
    assert_eq!(
        result,
        Bytes::from(vec![
            (0x00 << 6), // chunk basic header - fmt: 0, csid: 0
            0x00, // extended csid (64 + 0) = 64
            0x00, 0x00, 0x00, // timestamp (0)
            0x00, 0x00, 0x08, // message length (8 bytes)
            0x02, // message type id (abort)
            0x00, 0x00, 0x00, 0x00, // message stream id (0)
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, // message payload
        ])
    );
}

#[test]
fn test_encoder_extended_csid_ext() {
	let encoder = ChunkEncoder::default();
	let mut writer = BytesWriter::default();

	let chunk = Chunk::new(
		320,
		0,
		MessageTypeID::Abort,
		0,
		Bytes::from(vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07]),
	);

	encoder.write_chunk(&mut writer, chunk).unwrap();

	let result = writer.dispose();

	#[rustfmt::skip]
    assert_eq!(
        result,
        Bytes::from(vec![
            0x01, // chunk basic header - fmt: 0, csid: 1
            0x00, // extended csid (64 + 0) = 64
            0x01, // extended csid (256 * 1) = 256 + 64 + 0 = 320
            0x00, 0x00, 0x00, // timestamp (0)
            0x00, 0x00, 0x08, // message length (8 bytes)
            0x02, // message type id (abort)
            0x00, 0x00, 0x00, 0x00, // message stream id (0)
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, // message payload
        ])
    );
}
