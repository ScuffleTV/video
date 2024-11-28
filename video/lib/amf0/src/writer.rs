use std::collections::HashMap;
use std::io::Write;

use byteorder::{BigEndian, WriteBytesExt};
use bytesio::bytes_writer::BytesWriter;

use super::define::Amf0Marker;
use super::{Amf0Value, Amf0WriteError};

pub struct Amf0Writer;

impl Amf0Writer {
	pub fn write_any(writer: &mut BytesWriter, value: &Amf0Value) -> Result<(), Amf0WriteError> {
		match value {
			Amf0Value::Boolean(val) => Self::write_bool(writer, *val),
			Amf0Value::Null => Self::write_null(writer),
			Amf0Value::Number(val) => Self::write_number(writer, *val),
			Amf0Value::String(val) => Self::write_string(writer, val.as_str()),
			Amf0Value::Object(val) => Self::write_object(writer, val),
			_ => Err(Amf0WriteError::UnsupportedType(value.clone())),
		}
	}

	fn write_object_eof(writer: &mut BytesWriter) -> Result<(), Amf0WriteError> {
		writer.write_u24::<BigEndian>(Amf0Marker::ObjectEnd as u32)?;
		Ok(())
	}

	pub fn write_number(writer: &mut BytesWriter, value: f64) -> Result<(), Amf0WriteError> {
		writer.write_u8(Amf0Marker::Number as u8)?;
		writer.write_f64::<BigEndian>(value)?;
		Ok(())
	}

	pub fn write_bool(writer: &mut BytesWriter, value: bool) -> Result<(), Amf0WriteError> {
		writer.write_u8(Amf0Marker::Boolean as u8)?;
		writer.write_u8(value as u8)?;
		Ok(())
	}

	pub fn write_string(writer: &mut BytesWriter, value: &str) -> Result<(), Amf0WriteError> {
		if value.len() > (u16::MAX as usize) {
			return Err(Amf0WriteError::NormalStringTooLong);
		}
		writer.write_u8(Amf0Marker::String as u8)?;
		writer.write_u16::<BigEndian>(value.len() as u16)?;
		writer.write_all(value.as_bytes())?;
		Ok(())
	}

	pub fn write_null(writer: &mut BytesWriter) -> Result<(), Amf0WriteError> {
		writer.write_u8(Amf0Marker::Null as u8)?;
		Ok(())
	}

	pub fn write_object(writer: &mut BytesWriter, properties: &HashMap<String, Amf0Value>) -> Result<(), Amf0WriteError> {
		writer.write_u8(Amf0Marker::Object as u8)?;
		for (key, value) in properties {
			writer.write_u16::<BigEndian>(key.len() as u16)?;
			writer.write_all(key.as_bytes())?;
			Self::write_any(writer, value)?;
		}

		Self::write_object_eof(writer)?;
		Ok(())
	}
}
