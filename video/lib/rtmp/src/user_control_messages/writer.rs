use byteorder::{BigEndian, WriteBytesExt};
use bytesio::bytes_writer::BytesWriter;

use super::define;
use super::errors::EventMessagesError;
use crate::chunk::{Chunk, ChunkEncoder};
use crate::messages::MessageTypeID;

pub struct EventMessagesWriter;

impl EventMessagesWriter {
	pub fn write_stream_begin(
		encoder: &ChunkEncoder,
		writer: &mut BytesWriter,
		stream_id: u32,
	) -> Result<(), EventMessagesError> {
		let mut data = Vec::new();

		data.write_u16::<BigEndian>(define::RTMP_EVENT_STREAM_BEGIN)
			.expect("write u16");
		data.write_u32::<BigEndian>(stream_id).expect("write u32");

		encoder.write_chunk(writer, Chunk::new(0x02, 0, MessageTypeID::UserControlEvent, 0, data.into()))?;

		Ok(())
	}
}
