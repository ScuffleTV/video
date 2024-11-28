use std::io::Write;

use byteorder::{BigEndian, LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::bytes_reader::BytesReader;
use crate::bytes_writer::BytesWriter;

#[test]
fn test_byte_writer() {
	let mut writer = BytesWriter::default();
	writer.write_u8(0x01).unwrap(); // 1 byte
	writer.write_u16::<BigEndian>(0x0203).unwrap(); // 2 bytes
	writer.write_u24::<LittleEndian>(0x040506).unwrap(); // 3 bytes
	writer.write_u32::<BigEndian>(0x0708090a).unwrap(); // 4 bytes
	writer.write_f64::<LittleEndian>(0.123456789).unwrap(); // 8 bytes
	writer.write_all(&[0x0b, 0x0c, 0x0d, 0x0e, 0x0f]).unwrap(); // 5 bytes

	let bytes = writer.get_current_bytes();
	let mut reader = BytesReader::new(bytes);
	assert_eq!(reader.read_u8().unwrap(), 0x01);
	assert_eq!(reader.read_u16::<BigEndian>().unwrap(), 0x0203);
	assert_eq!(reader.read_u24::<LittleEndian>().unwrap(), 0x040506);
	assert_eq!(reader.read_u32::<BigEndian>().unwrap(), 0x0708090a);
	assert_eq!(reader.read_f64::<LittleEndian>().unwrap(), 0.123456789);
	assert_eq!(reader.read_bytes(5).unwrap().to_vec(), &[0x0b, 0x0c, 0x0d, 0x0e, 0x0f]);

	assert!(reader.is_empty());
	assert!(!writer.extract_current_bytes().is_empty())
}
