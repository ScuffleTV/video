use std::io::Cursor;

use byteorder::{BigEndian, ReadBytesExt};
use bytes::Bytes;

use super::errors::ProtocolControlMessageError;

pub struct ProtocolControlMessageReader;

impl ProtocolControlMessageReader {
	pub fn read_set_chunk_size(data: Bytes) -> Result<u32, ProtocolControlMessageError> {
		let mut cursor = Cursor::new(data);
		let chunk_size = cursor.read_u32::<BigEndian>()?;

		Ok(chunk_size)
	}
}
