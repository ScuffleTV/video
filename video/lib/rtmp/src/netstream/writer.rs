use std::collections::HashMap;

use amf0::{Amf0Value, Amf0Writer};
use bytesio::bytes_writer::BytesWriter;

use super::errors::NetStreamError;
use crate::chunk::{Chunk, ChunkEncoder, DefinedChunkStreamID};
use crate::messages::MessageTypeID;

pub struct NetStreamWriter {}

impl NetStreamWriter {
	fn write_chunk(
		encoder: &ChunkEncoder,
		amf0_writer: BytesWriter,
		writer: &mut BytesWriter,
	) -> Result<(), NetStreamError> {
		let data = amf0_writer.dispose();

		encoder.write_chunk(
			writer,
			Chunk::new(DefinedChunkStreamID::Command as u32, 0, MessageTypeID::CommandAMF0, 0, data),
		)?;

		Ok(())
	}

	pub fn write_on_status(
		encoder: &ChunkEncoder,
		writer: &mut BytesWriter,
		transaction_id: f64,
		level: &str,
		code: &str,
		description: &str,
	) -> Result<(), NetStreamError> {
		let mut amf0_writer = BytesWriter::default();

		Amf0Writer::write_string(&mut amf0_writer, "onStatus")?;
		Amf0Writer::write_number(&mut amf0_writer, transaction_id)?;
		Amf0Writer::write_null(&mut amf0_writer)?;
		Amf0Writer::write_object(
			&mut amf0_writer,
			&HashMap::from([
				("level".to_string(), Amf0Value::String(level.to_string())),
				("code".to_string(), Amf0Value::String(code.to_string())),
				("description".to_string(), Amf0Value::String(description.to_string())),
			]),
		)?;

		Self::write_chunk(encoder, amf0_writer, writer)
	}
}
