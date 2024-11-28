use std::io::{Read, Seek, SeekFrom};

use byteorder::ReadBytesExt;
use bytes::Bytes;

use crate::bit_reader::BitReader;

#[test]
fn test_bit_reader() {
	let data = Bytes::from(vec![0b10110111, 0b01011000]);

	let mut reader = BitReader::from(data);

	assert!(reader.read_bit().unwrap());
	assert!(!reader.read_bit().unwrap());
	assert!(reader.read_bit().unwrap());
	assert!(reader.read_bit().unwrap());

	assert_eq!(reader.read_bits(3).unwrap(), 0b011);
	assert_eq!(reader.read_bits(8).unwrap(), 0b10101100);
	assert_eq!(reader.read_bits(1).unwrap(), 0b0);

	assert!(reader.is_empty());
}

#[test]
fn test_bit_reader_read() {
	let data = Bytes::from(vec![0b10110111, 0b01011000, 0b11111111]);

	let mut reader = BitReader::from(data);

	reader.seek_bits(1).unwrap();

	let mut buf = [0u8; 2];
	assert_eq!(reader.read(&mut buf).unwrap(), 2);
	assert_eq!(buf, [0b01101110, 0b10110001]);
}

#[test]
fn test_bit_reader_read_ext() {
	let data = Bytes::from(vec![0b10110111, 0b01011000, 0b11111111]);

	let mut reader = BitReader::from(data);

	reader.seek_bits(1).unwrap();

	assert_eq!(reader.get_bit_pos(), 1);

	reader.seek_bits(3).unwrap();

	assert_eq!(reader.get_bit_pos(), 4);

	let mut buf = [0u8; 2];
	assert_eq!(reader.read(&mut buf).unwrap(), 2);
	assert_eq!(buf, [0b01110101, 0b10001111]);
}

#[test]
fn test_bit_reader_seek() {
	let data = Bytes::from(vec![0b10110111, 0b01011000, 0b11111111]);

	let mut reader = BitReader::from(data);

	reader.seek(SeekFrom::Start(1)).unwrap();

	assert!(!reader.read_bit().unwrap());
	assert!(reader.read_bit().unwrap());
	assert!(!reader.read_bit().unwrap());

	reader.seek_to(3).unwrap();
	reader.seek(SeekFrom::Current(1)).unwrap();

	assert_eq!(reader.read_u8().unwrap(), 0b11000111);

	reader.seek(SeekFrom::End(-1)).unwrap();

	assert_eq!(reader.read_u8().unwrap(), 0b11111111);
}

#[test]
fn test_bit_reader_align() {
	let data = Bytes::from(vec![0b10110111, 0b01011000, 0b11111111]);

	let mut reader = BitReader::from(data);

	reader.seek_bits(1).unwrap();

	reader.align().unwrap();

	assert!(reader.is_aligned());
	assert_eq!(reader.get_bit_pos(), 0);
}
