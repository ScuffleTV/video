use std::collections::HashMap;
use std::io::{Cursor, Seek, SeekFrom};

use byteorder::{BigEndian, ReadBytesExt};
use bytes::Bytes;
use num_traits::FromPrimitive;

use super::{Amf0Marker, Amf0ReadError, Amf0Value};

pub struct Amf0Reader {
	cursor: Cursor<Bytes>,
}

impl Amf0Reader {
	pub fn new(buff: Bytes) -> Self {
		Self {
			cursor: Cursor::new(buff),
		}
	}

	fn is_empty(&self) -> bool {
		self.cursor.get_ref().len() == self.cursor.position() as usize
	}

	fn read_bytes(&mut self, len: usize) -> Result<Bytes, Amf0ReadError> {
		let pos = self.cursor.position();
		self.cursor.seek(SeekFrom::Current(len as i64))?;
		Ok(self.cursor.get_ref().slice(pos as usize..pos as usize + len))
	}

	pub fn read_all(&mut self) -> Result<Vec<Amf0Value>, Amf0ReadError> {
		let mut results = vec![];

		loop {
			let result = self.read_any()?;

			match result {
				Amf0Value::ObjectEnd => {
					break;
				}
				_ => {
					results.push(result);
				}
			}
		}

		Ok(results)
	}

	pub fn read_any(&mut self) -> Result<Amf0Value, Amf0ReadError> {
		if self.is_empty() {
			return Ok(Amf0Value::ObjectEnd);
		}

		let marker = self.cursor.read_u8()?;
		let marker = Amf0Marker::from_u8(marker).ok_or_else(|| Amf0ReadError::UnknownMarker(marker))?;

		match marker {
			Amf0Marker::Number => self.read_number(),
			Amf0Marker::Boolean => self.read_bool(),
			Amf0Marker::String => self.read_string(),
			Amf0Marker::Object => self.read_object(),
			Amf0Marker::Null => self.read_null(),
			Amf0Marker::EcmaArray => self.read_ecma_array(),
			Amf0Marker::LongString => self.read_long_string(),
			_ => Err(Amf0ReadError::UnsupportedType(marker)),
		}
	}

	pub fn read_with_type(&mut self, specified_marker: Amf0Marker) -> Result<Amf0Value, Amf0ReadError> {
		let marker = self.cursor.read_u8()?;
		self.cursor.seek(SeekFrom::Current(-1))?; // seek back to the original position

		let marker = Amf0Marker::from_u8(marker).ok_or_else(|| Amf0ReadError::UnknownMarker(marker))?;
		if marker != specified_marker {
			return Err(Amf0ReadError::WrongType);
		}

		self.read_any()
	}

	pub fn read_number(&mut self) -> Result<Amf0Value, Amf0ReadError> {
		let number = self.cursor.read_f64::<BigEndian>()?;
		let value = Amf0Value::Number(number);
		Ok(value)
	}

	pub fn read_bool(&mut self) -> Result<Amf0Value, Amf0ReadError> {
		let value = self.cursor.read_u8()?;

		match value {
			1 => Ok(Amf0Value::Boolean(true)),
			_ => Ok(Amf0Value::Boolean(false)),
		}
	}

	fn read_raw_string(&mut self) -> Result<String, Amf0ReadError> {
		let l = self.cursor.read_u16::<BigEndian>()?;

		let bytes = self.read_bytes(l as usize)?;

		Ok(std::str::from_utf8(&bytes)?.to_string())
	}

	pub fn read_string(&mut self) -> Result<Amf0Value, Amf0ReadError> {
		let raw_string = self.read_raw_string()?;
		Ok(Amf0Value::String(raw_string))
	}

	pub fn read_null(&mut self) -> Result<Amf0Value, Amf0ReadError> {
		Ok(Amf0Value::Null)
	}

	pub fn is_read_object_eof(&mut self) -> Result<bool, Amf0ReadError> {
		let pos = self.cursor.position();
		let marker = self.cursor.read_u24::<BigEndian>();
		self.cursor.seek(SeekFrom::Start(pos))?;

		match Amf0Marker::from_u32(marker?) {
			Some(Amf0Marker::ObjectEnd) => {
				self.cursor.read_u24::<BigEndian>()?;
				Ok(true)
			}
			_ => Ok(false),
		}
	}

	pub fn read_object(&mut self) -> Result<Amf0Value, Amf0ReadError> {
		let mut properties = HashMap::new();

		loop {
			let is_eof = self.is_read_object_eof()?;

			if is_eof {
				break;
			}

			let key = self.read_raw_string()?;
			let val = self.read_any()?;

			properties.insert(key, val);
		}

		Ok(Amf0Value::Object(properties))
	}

	pub fn read_ecma_array(&mut self) -> Result<Amf0Value, Amf0ReadError> {
		let len = self.cursor.read_u32::<BigEndian>()?;

		let mut properties = HashMap::new();

		for _ in 0..len {
			let key = self.read_raw_string()?;
			let val = self.read_any()?;
			properties.insert(key, val);
		}

		// Sometimes the object end marker is present and sometimes it is not.
		// If it is there just read it, if not then we are done.
		self.is_read_object_eof().ok(); // ignore the result

		Ok(Amf0Value::Object(properties))
	}

	pub fn read_long_string(&mut self) -> Result<Amf0Value, Amf0ReadError> {
		let l = self.cursor.read_u32::<BigEndian>()?;

		let buff = self.read_bytes(l as usize)?;
		let val = std::str::from_utf8(&buff)?;

		Ok(Amf0Value::LongString(val.to_string()))
	}
}
