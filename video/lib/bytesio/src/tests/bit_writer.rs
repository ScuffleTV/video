use std::io::{Seek, SeekFrom, Write};

use crate::bit_writer::BitWriter;

#[test]
fn test_bit_writer() {
	let mut bit_writer = BitWriter::default();

	bit_writer.write_bits(0b1, 1).unwrap(); // 1
	bit_writer.write_bits(0b010, 3).unwrap(); // 4
	bit_writer.write_bits(0b011, 3).unwrap(); // 7
	bit_writer.write_bits(0b00100, 5).unwrap(); // 12
	bit_writer.write_bits(0b00101, 5).unwrap(); // 17

	let data = bit_writer.get_ref();

	// 2 bytes + 1 bit
	assert_eq!(data, &[0b10100110, 0b01000010, 0b10000000]);

	assert!(!bit_writer.is_aligned());

	bit_writer.write_bits(0b1111000, 7).unwrap(); // 24

	let data = bit_writer.get_ref();

	// 3 bytes
	assert_eq!(data, &[0b10100110, 0b01000010, 0b11111000]);

	assert!(bit_writer.is_aligned());

	bit_writer.write_bits(0b1111000, 7).unwrap(); // 31

	bit_writer.align().unwrap(); // 32

	let data = bit_writer.get_ref();

	// 4 bytes
	assert_eq!(data, &[0b10100110, 0b01000010, 0b11111000, 0b11110000]);

	assert!(bit_writer.is_aligned());

	bit_writer.write_bits(0b1, 1).unwrap(); // 33

	let data = bit_writer.get_ref();

	// 5 bytes
	assert_eq!(data, &[0b10100110, 0b01000010, 0b11111000, 0b11110000, 0b10000000]);
}

#[test]
fn test_bit_writer_write() {
	let mut bit_writer = BitWriter::default();

	bit_writer.write_bit(true).unwrap(); // 1
	bit_writer
		.write_all(&[0b00000001, 0b00000010, 0b00000011, 0b00000100])
		.unwrap(); // 33

	let data = bit_writer.get_ref();

	// 5 bytes
	assert_eq!(data, &[0b10000000, 0b10000001, 0b00000001, 0b10000010, 0b0,]);
}

#[test]
fn test_bit_writer_write_aligned() {
	let mut bit_writer = BitWriter::default();

	bit_writer.write_bit(true).unwrap(); // 1
	bit_writer.align().unwrap(); // 8
	bit_writer
		.write_all(&[0b00000001, 0b00000010, 0b00000011, 0b00000100])
		.unwrap(); // 40

	let data = bit_writer.get_ref();

	// 5 bytes
	assert_eq!(data, &[0b10000000, 0b00000001, 0b00000010, 0b00000011, 0b00000100,]);
}

#[test]
fn test_bit_writer_seek() {
	let mut bit_writer = BitWriter::default();

	bit_writer.write_bits(0b1, 1).unwrap(); // 1
	bit_writer.write_bits(0b010, 3).unwrap(); // 4
	bit_writer.write_bits(0b011, 3).unwrap(); // 7
	bit_writer.write_bits(0b0, 1).unwrap(); // 8

	let data = bit_writer.get_ref();

	// 1 byte
	assert_eq!(data, &[0b10100110]);

	bit_writer.seek(SeekFrom::Start(0)).unwrap();

	bit_writer.write_bits(0b1, 1).unwrap(); // 1
	bit_writer.write_bits(0b111, 3).unwrap(); // 4
	bit_writer.write_bits(0b111, 3).unwrap(); // 7
	bit_writer.write_bits(0b10100, 5).unwrap(); // 12

	let data = bit_writer.get_ref();

	// 1 bytes + 4 bits
	assert_eq!(data, &[0b11111111, 0b01000000]);

	bit_writer.seek_bits(-5);

	bit_writer.write_bits(0b0, 1).unwrap();

	let data = bit_writer.get_ref();

	// 1 bytes + 4 bits
	assert_eq!(data, &[0b11111110, 0b01000000]);

	bit_writer.seek_to(4);

	bit_writer.write_bits(0b0, 1).unwrap();

	let data = bit_writer.get_ref();

	// 1 bytes + 4 bits
	assert_eq!(data, &[0b11110110, 0b01000000]);
}
