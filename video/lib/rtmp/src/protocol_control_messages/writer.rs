use byteorder::{BigEndian, WriteBytesExt};
use bytes::Bytes;
use bytesio::bytes_writer::BytesWriter;

use super::errors::ProtocolControlMessageError;
use crate::chunk::{Chunk, ChunkEncoder};
use crate::messages::MessageTypeID;

pub struct ProtocolControlMessagesWriter;

impl ProtocolControlMessagesWriter {
	pub fn write_set_chunk_size(
		encoder: &ChunkEncoder,
		writer: &mut BytesWriter,
		chunk_size: u32, // 31 bits
	) -> Result<(), ProtocolControlMessageError> {
		// According to spec the first bit must be 0.
		let chunk_size = chunk_size & 0x7FFFFFFF; // 31 bits only

		encoder.write_chunk(
			writer,
			Chunk::new(
				2, // chunk stream must be 2
				0, // timestamps are ignored
				MessageTypeID::SetChunkSize,
				0, // message stream id is ignored
				Bytes::from(chunk_size.to_be_bytes().to_vec()),
			),
		)?;

		Ok(())
	}

	pub fn write_window_acknowledgement_size(
		encoder: &ChunkEncoder,
		writer: &mut BytesWriter,
		window_size: u32,
	) -> Result<(), ProtocolControlMessageError> {
		encoder.write_chunk(
			writer,
			Chunk::new(
				2, // chunk stream must be 2
				0, // timestamps are ignored
				MessageTypeID::WindowAcknowledgementSize,
				0, // message stream id is ignored
				Bytes::from(window_size.to_be_bytes().to_vec()),
			),
		)?;

		Ok(())
	}

	pub fn write_set_peer_bandwidth(
		encoder: &ChunkEncoder,
		writer: &mut BytesWriter,
		window_size: u32,
		limit_type: u8,
	) -> Result<(), ProtocolControlMessageError> {
		let mut data = Vec::new();
		data.write_u32::<BigEndian>(window_size).expect("Failed to write window size");
		data.write_u8(limit_type).expect("Failed to write limit type");

		encoder.write_chunk(
			writer,
			Chunk::new(
				2, // chunk stream must be 2
				0, // timestamps are ignored
				MessageTypeID::SetPeerBandwidth,
				0, // message stream id is ignored
				Bytes::from(data),
			),
		)?;

		Ok(())
	}
}
