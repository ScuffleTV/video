use std::collections::HashMap;

use amf0::{Amf0Value, Amf0Writer};
use bytesio::bytes_writer::BytesWriter;

use super::errors::NetConnectionError;
use crate::chunk::{Chunk, ChunkEncoder, DefinedChunkStreamID};
use crate::messages::MessageTypeID;

pub struct NetConnection;

impl NetConnection {
	fn write_chunk(encoder: &ChunkEncoder, amf0: BytesWriter, writer: &mut BytesWriter) -> Result<(), NetConnectionError> {
		let data = amf0.dispose();

		encoder.write_chunk(
			writer,
			Chunk::new(DefinedChunkStreamID::Command as u32, 0, MessageTypeID::CommandAMF0, 0, data),
		)?;

		Ok(())
	}

	#[allow(clippy::too_many_arguments)]
	pub fn write_connect_response(
		encoder: &ChunkEncoder,
		writer: &mut BytesWriter,
		transaction_id: f64,
		fmsver: &str,
		capabilities: f64,
		code: &str,
		level: &str,
		description: &str,
		encoding: f64,
	) -> Result<(), NetConnectionError> {
		let mut amf0_writer = BytesWriter::default();

		Amf0Writer::write_string(&mut amf0_writer, "_result")?;
		Amf0Writer::write_number(&mut amf0_writer, transaction_id)?;
		Amf0Writer::write_object(
			&mut amf0_writer,
			&HashMap::from([
				("fmsVer".to_string(), Amf0Value::String(fmsver.to_string())),
				("capabilities".to_string(), Amf0Value::Number(capabilities)),
			]),
		)?;
		Amf0Writer::write_object(
			&mut amf0_writer,
			&HashMap::from([
				("level".to_string(), Amf0Value::String(level.to_string())),
				("code".to_string(), Amf0Value::String(code.to_string())),
				("description".to_string(), Amf0Value::String(description.to_string())),
				("objectEncoding".to_string(), Amf0Value::Number(encoding)),
			]),
		)?;

		Self::write_chunk(encoder, amf0_writer, writer)
	}

	pub fn write_create_stream_response(
		encoder: &ChunkEncoder,
		writer: &mut BytesWriter,
		transaction_id: f64,
		stream_id: f64,
	) -> Result<(), NetConnectionError> {
		let mut amf0_writer = BytesWriter::default();

		Amf0Writer::write_string(&mut amf0_writer, "_result")?;
		Amf0Writer::write_number(&mut amf0_writer, transaction_id)?;
		Amf0Writer::write_null(&mut amf0_writer)?;
		Amf0Writer::write_number(&mut amf0_writer, stream_id)?;

		Self::write_chunk(encoder, amf0_writer, writer)
	}
}
