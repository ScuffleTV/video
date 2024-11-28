use bytesio::bit_reader::BitReader;
use bytesio::bit_writer::BitWriter;

use crate::{read_exp_golomb, read_signed_exp_golomb, write_exp_golomb, write_signed_exp_golomb};

#[test]
fn test_exp_glob_decode() {
	let mut bit_writer = BitWriter::default();

	bit_writer.write_bits(0b1, 1).unwrap(); // 0
	bit_writer.write_bits(0b010, 3).unwrap(); // 1
	bit_writer.write_bits(0b011, 3).unwrap(); // 2
	bit_writer.write_bits(0b00100, 5).unwrap(); // 3
	bit_writer.write_bits(0b00101, 5).unwrap(); // 4
	bit_writer.write_bits(0b00110, 5).unwrap(); // 5
	bit_writer.write_bits(0b00111, 5).unwrap(); // 6

	let data = bit_writer.into_inner();

	let mut bit_reader = BitReader::from(data);

	let remaining_bits = bit_reader.remaining_bits();

	let result = read_exp_golomb(&mut bit_reader).unwrap();
	assert_eq!(result, 0);
	assert_eq!(bit_reader.remaining_bits(), remaining_bits - 1);

	let result = read_exp_golomb(&mut bit_reader).unwrap();
	assert_eq!(result, 1);
	assert_eq!(bit_reader.remaining_bits(), remaining_bits - 4);

	let result = read_exp_golomb(&mut bit_reader).unwrap();
	assert_eq!(result, 2);
	assert_eq!(bit_reader.remaining_bits(), remaining_bits - 7);

	let result = read_exp_golomb(&mut bit_reader).unwrap();
	assert_eq!(result, 3);
	assert_eq!(bit_reader.remaining_bits(), remaining_bits - 12);

	let result = read_exp_golomb(&mut bit_reader).unwrap();
	assert_eq!(result, 4);
	assert_eq!(bit_reader.remaining_bits(), remaining_bits - 17);

	let result = read_exp_golomb(&mut bit_reader).unwrap();
	assert_eq!(result, 5);
	assert_eq!(bit_reader.remaining_bits(), remaining_bits - 22);

	let result = read_exp_golomb(&mut bit_reader).unwrap();
	assert_eq!(result, 6);
	assert_eq!(bit_reader.remaining_bits(), remaining_bits - 27);
}

#[test]
fn test_signed_exp_glob_decode() {
	let mut bit_writer = BitWriter::default();

	bit_writer.write_bits(0b1, 1).unwrap(); // 0
	bit_writer.write_bits(0b010, 3).unwrap(); // 1
	bit_writer.write_bits(0b011, 3).unwrap(); // -1
	bit_writer.write_bits(0b00100, 5).unwrap(); // 2
	bit_writer.write_bits(0b00101, 5).unwrap(); // -2
	bit_writer.write_bits(0b00110, 5).unwrap(); // 3
	bit_writer.write_bits(0b00111, 5).unwrap(); // -3

	let data = bit_writer.into_inner();

	let mut bit_reader = BitReader::from(data);

	let remaining_bits = bit_reader.remaining_bits();

	let result = read_signed_exp_golomb(&mut bit_reader).unwrap();
	assert_eq!(result, 0);
	assert_eq!(bit_reader.remaining_bits(), remaining_bits - 1);

	let result = read_signed_exp_golomb(&mut bit_reader).unwrap();
	assert_eq!(result, 1);
	assert_eq!(bit_reader.remaining_bits(), remaining_bits - 4);

	let result = read_signed_exp_golomb(&mut bit_reader).unwrap();
	assert_eq!(result, -1);
	assert_eq!(bit_reader.remaining_bits(), remaining_bits - 7);

	let result = read_signed_exp_golomb(&mut bit_reader).unwrap();
	assert_eq!(result, 2);
	assert_eq!(bit_reader.remaining_bits(), remaining_bits - 12);

	let result = read_signed_exp_golomb(&mut bit_reader).unwrap();
	assert_eq!(result, -2);
	assert_eq!(bit_reader.remaining_bits(), remaining_bits - 17);

	let result = read_signed_exp_golomb(&mut bit_reader).unwrap();
	assert_eq!(result, 3);
	assert_eq!(bit_reader.remaining_bits(), remaining_bits - 22);

	let result = read_signed_exp_golomb(&mut bit_reader).unwrap();
	assert_eq!(result, -3);
	assert_eq!(bit_reader.remaining_bits(), remaining_bits - 27);
}

#[test]
fn test_exp_glob_encode() {
	let mut bit_writer = BitWriter::default();

	write_exp_golomb(&mut bit_writer, 0).unwrap();
	write_exp_golomb(&mut bit_writer, 1).unwrap();
	write_exp_golomb(&mut bit_writer, 2).unwrap();
	write_exp_golomb(&mut bit_writer, 3).unwrap();
	write_exp_golomb(&mut bit_writer, 4).unwrap();
	write_exp_golomb(&mut bit_writer, 5).unwrap();
	write_exp_golomb(&mut bit_writer, 6).unwrap();
	write_exp_golomb(&mut bit_writer, u64::MAX - 1).unwrap();

	let data = bit_writer.into_inner();

	let mut bit_reader = BitReader::from(data);

	let remaining_bits = bit_reader.remaining_bits();

	let result = read_exp_golomb(&mut bit_reader).unwrap();
	assert_eq!(result, 0);
	assert_eq!(bit_reader.remaining_bits(), remaining_bits - 1);

	let result = read_exp_golomb(&mut bit_reader).unwrap();
	assert_eq!(result, 1);
	assert_eq!(bit_reader.remaining_bits(), remaining_bits - 4);

	let result = read_exp_golomb(&mut bit_reader).unwrap();
	assert_eq!(result, 2);
	assert_eq!(bit_reader.remaining_bits(), remaining_bits - 7);

	let result = read_exp_golomb(&mut bit_reader).unwrap();
	assert_eq!(result, 3);
	assert_eq!(bit_reader.remaining_bits(), remaining_bits - 12);

	let result = read_exp_golomb(&mut bit_reader).unwrap();
	assert_eq!(result, 4);
	assert_eq!(bit_reader.remaining_bits(), remaining_bits - 17);

	let result = read_exp_golomb(&mut bit_reader).unwrap();
	assert_eq!(result, 5);
	assert_eq!(bit_reader.remaining_bits(), remaining_bits - 22);

	let result = read_exp_golomb(&mut bit_reader).unwrap();
	assert_eq!(result, 6);
	assert_eq!(bit_reader.remaining_bits(), remaining_bits - 27);

	let result = read_exp_golomb(&mut bit_reader).unwrap();
	assert_eq!(result, u64::MAX - 1);
	assert_eq!(bit_reader.remaining_bits(), remaining_bits - 154);
}

#[test]
fn test_signed_exp_glob_encode() {
	let mut bit_writer = BitWriter::default();

	write_signed_exp_golomb(&mut bit_writer, 0).unwrap();
	write_signed_exp_golomb(&mut bit_writer, 1).unwrap();
	write_signed_exp_golomb(&mut bit_writer, -1).unwrap();
	write_signed_exp_golomb(&mut bit_writer, 2).unwrap();
	write_signed_exp_golomb(&mut bit_writer, -2).unwrap();
	write_signed_exp_golomb(&mut bit_writer, 3).unwrap();
	write_signed_exp_golomb(&mut bit_writer, -3).unwrap();
	write_signed_exp_golomb(&mut bit_writer, i64::MAX).unwrap();

	let data = bit_writer.into_inner();

	let mut bit_reader = BitReader::from(data);

	let remaining_bits = bit_reader.remaining_bits();

	let result = read_signed_exp_golomb(&mut bit_reader).unwrap();
	assert_eq!(result, 0);
	assert_eq!(bit_reader.remaining_bits(), remaining_bits - 1);

	let result = read_signed_exp_golomb(&mut bit_reader).unwrap();
	assert_eq!(result, 1);
	assert_eq!(bit_reader.remaining_bits(), remaining_bits - 4);

	let result = read_signed_exp_golomb(&mut bit_reader).unwrap();
	assert_eq!(result, -1);
	assert_eq!(bit_reader.remaining_bits(), remaining_bits - 7);

	let result = read_signed_exp_golomb(&mut bit_reader).unwrap();
	assert_eq!(result, 2);
	assert_eq!(bit_reader.remaining_bits(), remaining_bits - 12);

	let result = read_signed_exp_golomb(&mut bit_reader).unwrap();
	assert_eq!(result, -2);
	assert_eq!(bit_reader.remaining_bits(), remaining_bits - 17);

	let result = read_signed_exp_golomb(&mut bit_reader).unwrap();
	assert_eq!(result, 3);
	assert_eq!(bit_reader.remaining_bits(), remaining_bits - 22);

	let result = read_signed_exp_golomb(&mut bit_reader).unwrap();
	assert_eq!(result, -3);
	assert_eq!(bit_reader.remaining_bits(), remaining_bits - 27);

	let result = read_signed_exp_golomb(&mut bit_reader).unwrap();
	assert_eq!(result, i64::MAX);
	assert_eq!(bit_reader.remaining_bits(), remaining_bits - 154);
}
