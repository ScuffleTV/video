use std::io::{Cursor, Write};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;
use bytesio::bytes_reader::BytesCursor;
use bytesio::bytes_writer::BytesWriter;

use super::{HandshakeError, HandshakeServer};
use crate::handshake::define::{
	SchemaVersion, {self},
};
use crate::handshake::digest::DigestProcessor;
use crate::handshake::errors::DigestError;
use crate::handshake::ServerHandshakeState;

#[test]
fn test_simple_handshake() {
	let mut handshake_server = HandshakeServer::default();

	let mut c0c1 = Cursor::new(Vec::new());
	c0c1.write_u8(3).unwrap(); // version
	c0c1.write_u32::<BigEndian>(123).unwrap(); // timestamp
	c0c1.write_u32::<BigEndian>(0).unwrap(); // zero

	let mut write_client_random = vec![0; 1528];
	for (i, v) in write_client_random.iter_mut().enumerate() {
		*v = (i % 256) as u8;
	}

	c0c1.write_all(&write_client_random).unwrap();

	handshake_server.extend_data(&c0c1.into_inner());

	let mut writer = BytesWriter::default();
	handshake_server.handshake(&mut writer).unwrap();

	let mut reader = Cursor::new(writer.dispose());
	assert_eq!(reader.read_u8().unwrap(), 3); // version
	let timestamp = reader.read_u32::<BigEndian>().unwrap(); // timestamp
	assert_eq!(reader.read_u32::<BigEndian>().unwrap(), 0); // zero

	let server_random = reader.read_slice(1528).unwrap();

	assert_eq!(reader.read_u32::<BigEndian>().unwrap(), 123); // our timestamp
	let timestamp2 = reader.read_u32::<BigEndian>().unwrap(); // server timestamp

	assert!(timestamp2 >= timestamp);

	let read_client_random = reader.read_slice(1528).unwrap();

	assert_eq!(&write_client_random, &read_client_random);

	let mut c2 = Cursor::new(Vec::new());
	c2.write_u32::<BigEndian>(timestamp).unwrap(); // timestamp
	c2.write_u32::<BigEndian>(124).unwrap(); // our timestamp
	c2.write_all(&server_random).unwrap();

	handshake_server.extend_data(&c2.into_inner());

	let mut writer = BytesWriter::default();
	handshake_server.handshake(&mut writer).unwrap();

	assert_eq!(handshake_server.state(), ServerHandshakeState::Finish)
}

#[test]
fn test_complex_handshake() {
	let mut handshake_server = HandshakeServer::default();

	handshake_server.extend_data(&[3]); // version

	let mut c0c1 = Cursor::new(Vec::new());
	c0c1.write_u32::<BigEndian>(123).unwrap(); // timestamp
	c0c1.write_u32::<BigEndian>(100).unwrap(); // client version

	for i in 0..1528 {
		c0c1.write_u8((i % 256) as u8).unwrap();
	}

	let data_digest = DigestProcessor::new(
		Bytes::from(c0c1.into_inner()),
		Bytes::from_static(define::RTMP_CLIENT_KEY_FIRST_HALF.as_bytes()),
	);

	let (first, second, third) = data_digest.generate_and_fill_digest(SchemaVersion::Schema1).unwrap();

	// We need to create the digest of the client random

	handshake_server.extend_data(&first);
	handshake_server.extend_data(&second);
	handshake_server.extend_data(&third);

	let mut writer = BytesWriter::default();
	handshake_server.handshake(&mut writer).unwrap();

	let bytes = writer.dispose();

	let s0 = bytes.slice(0..1);
	let s1 = bytes.slice(1..1537);
	let s2 = bytes.slice(1537..3073);

	assert_eq!(s0[0], 3); // version
	assert_ne!((&s1[..4]).read_u32::<BigEndian>().unwrap(), 0); // timestamp should not be zero
	assert_eq!((&s1[4..8]).read_u32::<BigEndian>().unwrap(), define::RTMP_SERVER_VERSION); // RTMP version

	let data_digest = DigestProcessor::new(s1, Bytes::from_static(define::RTMP_SERVER_KEY_FIRST_HALF.as_bytes()));

	let (digest, schema) = data_digest.read_digest().unwrap();
	assert_eq!(schema, SchemaVersion::Schema1);

	assert_ne!((&s2[..4]).read_u32::<BigEndian>().unwrap(), 0); // timestamp should not be zero
	assert_eq!((&s2[4..8]).read_u32::<BigEndian>().unwrap(), 123); // our timestamp

	let key_digest = DigestProcessor::new(Bytes::new(), Bytes::from_static(&define::RTMP_SERVER_KEY));

	let data_digest = DigestProcessor::new(Bytes::new(), key_digest.make_digest(&second, &[]).unwrap());

	assert_eq!(data_digest.make_digest(&s2[..1504], &[]).unwrap(), s2.slice(1504..));

	let data_digest = DigestProcessor::new(Bytes::new(), key_digest.make_digest(&digest, &[]).unwrap());

	let mut c2 = Vec::new();
	for i in 0..1528 {
		c2.write_u8((i % 256) as u8).unwrap();
	}

	let digest = data_digest.make_digest(&c2, &[]).unwrap();

	handshake_server.extend_data(&c2);
	handshake_server.extend_data(&digest);

	let mut writer = BytesWriter::default();
	handshake_server.handshake(&mut writer).unwrap();

	assert_eq!(handshake_server.state(), ServerHandshakeState::Finish)
}

#[test]
fn test_error_display() {
	let err = HandshakeError::Digest(DigestError::CannotGenerate);
	assert_eq!(err.to_string(), "digest error: cannot generate digest");

	let err = HandshakeError::Digest(DigestError::DigestLengthNotCorrect);
	assert_eq!(err.to_string(), "digest error: digest length not correct");

	let err = HandshakeError::Digest(DigestError::UnknownSchema);
	assert_eq!(err.to_string(), "digest error: unknown schema");

	let err = HandshakeError::Digest(DigestError::NotEnoughData);
	assert_eq!(err.to_string(), "digest error: not enough data");

	let err = HandshakeError::IO(Cursor::new(Vec::<u8>::new()).read_u8().unwrap_err());
	// no idea why this io error is the error we get but this is mainly testing the
	// display impl anyway
	assert_eq!(err.to_string(), "io error: failed to fill whole buffer");
}
