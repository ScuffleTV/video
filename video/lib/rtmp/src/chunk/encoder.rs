use std::io::Write;

use byteorder::{BigEndian, LittleEndian, WriteBytesExt};
use bytesio::bytes_writer::BytesWriter;

use super::define::{Chunk, ChunkMessageHeader, ChunkType, INIT_CHUNK_SIZE};
use super::errors::ChunkEncodeError;

pub struct ChunkEncoder {
	chunk_size: usize,
}

impl Default for ChunkEncoder {
	fn default() -> Self {
		Self {
			chunk_size: INIT_CHUNK_SIZE,
		}
	}
}

impl ChunkEncoder {
	pub fn set_chunk_size(&mut self, chunk_size: usize) {
		self.chunk_size = chunk_size;
	}

	/// Internal function to write the basic header.
	fn write_basic_header(writer: &mut BytesWriter, fmt: ChunkType, csid: u32) -> Result<(), ChunkEncodeError> {
		let fmt = fmt as u8;

		if csid >= 64 + 255 {
			writer.write_u8(fmt << 6 | 1)?;
			let csid = csid - 64;

			let div = csid / 256;
			let rem = csid % 256;

			writer.write_u8(rem as u8)?;
			writer.write_u8(div as u8)?;
		} else if csid >= 64 {
			writer.write_u8(fmt << 6)?;
			writer.write_u8((csid - 64) as u8)?;
		} else {
			writer.write_u8(fmt << 6 | csid as u8)?;
		}

		Ok(())
	}

	fn write_message_header(writer: &mut BytesWriter, message_header: &ChunkMessageHeader) -> Result<(), ChunkEncodeError> {
		let timestamp = if message_header.timestamp >= 0xFFFFFF {
			0xFFFFFF
		} else {
			message_header.timestamp
		};

		writer.write_u24::<BigEndian>(timestamp)?;
		writer.write_u24::<BigEndian>(message_header.msg_length)?;
		writer.write_u8(message_header.msg_type_id as u8)?;
		writer.write_u32::<LittleEndian>(message_header.msg_stream_id)?;

		if message_header.is_extended_timestamp() {
			Self::write_extened_timestamp(writer, message_header.timestamp)?;
		}

		Ok(())
	}

	fn write_extened_timestamp(writer: &mut BytesWriter, timestamp: u32) -> Result<(), ChunkEncodeError> {
		writer.write_u32::<BigEndian>(timestamp)?;

		Ok(())
	}

	pub fn write_chunk(&self, writer: &mut BytesWriter, mut chunk_info: Chunk) -> Result<(), ChunkEncodeError> {
		Self::write_basic_header(writer, ChunkType::Type0, chunk_info.basic_header.chunk_stream_id)?;

		Self::write_message_header(writer, &chunk_info.message_header)?;

		while !chunk_info.payload.is_empty() {
			let cur_payload_size = if chunk_info.payload.len() > self.chunk_size {
				self.chunk_size
			} else {
				chunk_info.payload.len()
			};

			let payload_bytes = chunk_info.payload.split_to(cur_payload_size);
			writer.write_all(&payload_bytes[..])?;

			if !chunk_info.payload.is_empty() {
				Self::write_basic_header(writer, ChunkType::Type3, chunk_info.basic_header.chunk_stream_id)?;

				if chunk_info.message_header.is_extended_timestamp() {
					Self::write_extened_timestamp(writer, chunk_info.message_header.timestamp)?;
				}
			}
		}

		Ok(())
	}
}
