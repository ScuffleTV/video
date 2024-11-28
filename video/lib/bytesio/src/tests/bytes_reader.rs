use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use bytes::BytesMut;

use crate::bytes_reader::BytesReader;

#[test]
fn test_byte_reader() {
	let mut reader = BytesReader::new(BytesMut::from(&b"hello world"[..]));
	assert_eq!(reader.read_bytes(5).unwrap(), BytesMut::from(&b"hello"[..]));
	assert_eq!(reader.read_bytes(5).unwrap(), BytesMut::from(&b" worl"[..]));
	assert!(reader.read_bytes(5).is_err());
	assert_eq!(reader.read_bytes(1).unwrap(), BytesMut::from(&b"d"[..]));

	reader.extend_from_slice(&b"hello world"[..]);

	assert_eq!(reader.advance_bytes(5).unwrap(), &b"hello"[..]);
	assert_eq!(reader.read_bytes(11).unwrap(), &b"hello world"[..]);

	assert!(reader.is_empty());
}

#[test]
fn test_read_binary() {
	let binary: Vec<u8> = vec![54, 0, 15, 0, 255, 0, 255, 0];
	let mut reader = BytesReader::new(BytesMut::from(binary.as_slice()));
	assert_eq!(reader.read_u32::<BigEndian>().unwrap(), 905973504);

	let mut reader = BytesReader::new(BytesMut::from(binary.as_slice()));
	assert_eq!(reader.read_u32::<LittleEndian>().unwrap(), 983094);

	let mut reader = BytesReader::new(BytesMut::from(binary.as_slice()));
	assert_eq!(reader.read_u24::<BigEndian>().unwrap(), 3538959);

	let mut reader = BytesReader::new(BytesMut::from(binary.as_slice()));
	assert_eq!(reader.read_u24::<LittleEndian>().unwrap(), 983094);

	let mut reader = BytesReader::new(BytesMut::from(binary.as_slice()));
	assert_eq!(reader.read_u16::<BigEndian>().unwrap(), 13824);

	let mut reader = BytesReader::new(BytesMut::from(binary.as_slice()));
	assert_eq!(reader.read_u16::<LittleEndian>().unwrap(), 54);

	let mut reader = BytesReader::new(BytesMut::from(binary.as_slice()));
	assert_eq!(reader.read_u8().unwrap(), 54);

	let mut reader = BytesReader::new(BytesMut::from(binary.as_slice()));
	assert_eq!(reader.read_f64::<BigEndian>().unwrap(), 1.3734682653814624e-48);

	let mut reader = BytesReader::new(BytesMut::from(binary.as_slice()));
	assert_eq!(reader.read_f64::<LittleEndian>().unwrap(), 7.064161010106551e-304);
}

#[test]
fn test_get_index() {
	let binary: Vec<u8> = vec![54, 0, 15, 0, 255, 0, 255, 0];
	let reader = BytesReader::new(BytesMut::from(binary.as_slice()));
	assert_eq!(reader.get(0).unwrap(), 54);
	assert_eq!(reader.get(1).unwrap(), 0);
	assert_eq!(reader.get(2).unwrap(), 15);
	assert_eq!(reader.get(3).unwrap(), 0);
	assert_eq!(reader.get(4).unwrap(), 255);
	assert_eq!(reader.get(5).unwrap(), 0);
	assert_eq!(reader.get(6).unwrap(), 255);
	assert_eq!(reader.get(7).unwrap(), 0);
	assert!(reader.get(8).is_err());

	assert_eq!(reader.len(), 8);
}

#[test]
fn test_remaining() {
	let binary: Vec<u8> = vec![54, 0, 15, 0, 255, 0, 255, 0];
	let mut reader = BytesReader::new(BytesMut::from(binary.as_slice()));
	assert_eq!(reader.get_remaining_bytes(), &b"6\0\x0f\0\xff\0\xff\0"[..]);

	reader.read_bytes(4).unwrap();

	assert_eq!(reader.get_remaining_bytes(), &b"\xff\0\xff\0"[..]);

	assert_eq!(reader.extract_remaining_bytes(), &b"\xff\0\xff\0"[..]);

	assert_eq!(reader.get_remaining_bytes(), &b""[..]);
	assert_eq!(reader.len(), 0);
}
