use std::collections::HashMap;
use std::io::Cursor;

use byteorder::ReadBytesExt;
use bytesio::bytes_writer::BytesWriter;

use crate::{Amf0Marker, Amf0ReadError, Amf0Reader, Amf0Value, Amf0WriteError, Amf0Writer};

#[test]
fn test_reader_bool() {
	let amf0_bool = vec![0x01, 0x01]; // true
	let mut amf_reader = Amf0Reader::new(amf0_bool.into());
	let value = amf_reader.read_with_type(Amf0Marker::Boolean).unwrap();
	assert_eq!(value, Amf0Value::Boolean(true));
}

#[test]
fn test_reader_number() {
	let mut amf0_number = vec![0x00];
	amf0_number.extend_from_slice(&772.161_f64.to_be_bytes());

	let mut amf_reader = Amf0Reader::new(amf0_number.into());
	let value = amf_reader.read_with_type(Amf0Marker::Number).unwrap();
	assert_eq!(value, Amf0Value::Number(772.161));
}

#[test]
fn test_reader_string() {
	let mut amf0_string = vec![0x02, 0x00, 0x0b]; // 11 bytes
	amf0_string.extend_from_slice(b"Hello World");

	let mut amf_reader = Amf0Reader::new(amf0_string.into());
	let value = amf_reader.read_with_type(Amf0Marker::String).unwrap();
	assert_eq!(value, Amf0Value::String("Hello World".to_string()));
}

#[test]
fn test_reader_long_string() {
	let mut amf0_string = vec![0x0c, 0x00, 0x00, 0x00, 0x0b]; // 11 bytes
	amf0_string.extend_from_slice(b"Hello World");

	let mut amf_reader = Amf0Reader::new(amf0_string.into());
	let value = amf_reader.read_with_type(Amf0Marker::LongString).unwrap();
	assert_eq!(value, Amf0Value::LongString("Hello World".to_string()));
}

#[test]
fn test_reader_object() {
	let mut amf0_object = vec![0x03, 0x00, 0x04]; // 1 property with 4 bytes
	amf0_object.extend_from_slice(b"test");
	amf0_object.extend_from_slice(&[0x05]); // null
	amf0_object.extend_from_slice(&[0x00, 0x00, 0x09]); // object end (0x00 0x00 0x09)

	let mut amf_reader = Amf0Reader::new(amf0_object.into());
	let value = amf_reader.read_with_type(Amf0Marker::Object).unwrap();

	assert_eq!(
		value,
		Amf0Value::Object(HashMap::from([("test".to_string(), Amf0Value::Null)]))
	);
}

#[test]
fn test_reader_ecma_array() {
	let mut amf0_object = vec![0x08, 0x00, 0x00, 0x00, 0x01]; // 1 property
	amf0_object.extend_from_slice(&[0x00, 0x04]); // 4 bytes
	amf0_object.extend_from_slice(b"test");
	amf0_object.extend_from_slice(&[0x05]); // null

	let mut amf_reader = Amf0Reader::new(amf0_object.into());
	let value = amf_reader.read_with_type(Amf0Marker::EcmaArray).unwrap();

	assert_eq!(
		value,
		Amf0Value::Object(HashMap::from([("test".to_string(), Amf0Value::Null)]))
	);
}

#[test]
fn test_reader_multi_value() {
	let mut amf0_multi = vec![0x00];
	amf0_multi.extend_from_slice(&772.161_f64.to_be_bytes());
	amf0_multi.extend_from_slice(&[0x01, 0x01]); // true
	amf0_multi.extend_from_slice(&[0x02, 0x00, 0x0b]); // 11 bytes
	amf0_multi.extend_from_slice(b"Hello World");
	amf0_multi.extend_from_slice(&[0x03, 0x00, 0x04]); // 1 property with 4 bytes
	amf0_multi.extend_from_slice(b"test");
	amf0_multi.extend_from_slice(&[0x05]); // null
	amf0_multi.extend_from_slice(&[0x00, 0x00, 0x09]); // object end (0x00 0x00 0x09)

	let mut amf_reader = Amf0Reader::new(amf0_multi.into());
	let values = amf_reader.read_all().unwrap();

	assert_eq!(values.len(), 4);

	assert_eq!(values[0], Amf0Value::Number(772.161));
	assert_eq!(values[1], Amf0Value::Boolean(true));
	assert_eq!(values[2], Amf0Value::String("Hello World".to_string()));
	assert_eq!(
		values[3],
		Amf0Value::Object(HashMap::from([("test".to_string(), Amf0Value::Null)]))
	);
}

#[test]
fn test_read_error_display() {
	assert_eq!(Amf0ReadError::UnknownMarker(100).to_string(), "unknown marker: 100");

	assert_eq!(
		Amf0ReadError::UnsupportedType(Amf0Marker::Reference).to_string(),
		"unsupported type: Reference"
	);

	assert_eq!(Amf0ReadError::WrongType.to_string(), "wrong type");

	assert_eq!(
		Amf0ReadError::StringParseError(
			#[allow(unknown_lints, invalid_from_utf8)]
			std::str::from_utf8(b"\xFF\xFF").unwrap_err()
		)
		.to_string(),
		"string parse error: invalid utf-8 sequence of 1 bytes from index 0"
	);

	assert_eq!(
		Amf0ReadError::IO(Cursor::new(Vec::<u8>::new()).read_u8().unwrap_err()).to_string(),
		"io error: failed to fill whole buffer"
	);
}

#[test]
fn test_write_error_display() {
	assert_eq!(
		Amf0WriteError::UnsupportedType(Amf0Value::ObjectEnd).to_string(),
		"unsupported type: ObjectEnd"
	);

	assert_eq!(
		Amf0WriteError::IO(Cursor::new(Vec::<u8>::new()).read_u8().unwrap_err()).to_string(),
		"io error: failed to fill whole buffer"
	);

	assert_eq!(Amf0WriteError::NormalStringTooLong.to_string(), "normal string too long");
}

#[test]
fn test_write_number() {
	let mut amf0_number = vec![0x00];
	amf0_number.extend_from_slice(&772.161_f64.to_be_bytes());

	let mut writer = BytesWriter::default();

	Amf0Writer::write_number(&mut writer, 772.161).unwrap();

	assert_eq!(writer.dispose(), amf0_number);
}

#[test]
fn test_write_boolean() {
	let amf0_boolean = vec![0x01, 0x01];

	let mut writer = BytesWriter::default();

	Amf0Writer::write_bool(&mut writer, true).unwrap();

	assert_eq!(writer.dispose(), amf0_boolean);
}

#[test]
fn test_write_string() {
	let mut amf0_string = vec![0x02, 0x00, 0x0b];
	amf0_string.extend_from_slice(b"Hello World");

	let mut writer = BytesWriter::default();

	Amf0Writer::write_string(&mut writer, "Hello World").unwrap();

	assert_eq!(writer.dispose(), amf0_string);
}

#[test]
fn test_write_null() {
	let amf0_null = vec![0x05];

	let mut writer = BytesWriter::default();

	Amf0Writer::write_null(&mut writer).unwrap();

	assert_eq!(writer.dispose(), amf0_null);
}

#[test]
fn test_write_object() {
	let mut amf0_object = vec![0x03, 0x00, 0x04];
	amf0_object.extend_from_slice(b"test");
	amf0_object.extend_from_slice(&[0x05]);
	amf0_object.extend_from_slice(&[0x00, 0x00, 0x09]);

	let mut writer = BytesWriter::default();

	Amf0Writer::write_object(&mut writer, &HashMap::from([("test".to_string(), Amf0Value::Null)])).unwrap();

	assert_eq!(writer.dispose(), amf0_object);
}
