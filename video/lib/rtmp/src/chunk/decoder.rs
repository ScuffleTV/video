use std::cmp::min;
use std::collections::HashMap;
use std::io::{Cursor, Seek, SeekFrom};

use byteorder::{BigEndian, LittleEndian, ReadBytesExt};
use bytes::BytesMut;
use bytesio::bytes_reader::BytesReader;
use num_traits::FromPrimitive;

use super::define::{Chunk, ChunkBasicHeader, ChunkMessageHeader, ChunkType, INIT_CHUNK_SIZE, MAX_CHUNK_SIZE};
use super::errors::ChunkDecodeError;
use crate::messages::MessageTypeID;

// These constants are used to limit the amount of memory we use for partial
// chunks on normal operations we should never hit these limits
// This is for when someone is trying to send us a malicious chunk streams
const MAX_PARTIAL_CHUNK_SIZE: usize = 10 * 1024 * 1024; // 10MB (should be more than enough)
const MAX_PREVIOUS_CHUNK_HEADERS: usize = 100; // 100 chunks
const MAX_PARTIAL_CHUNK_COUNT: usize = 4; // 4 chunks

pub struct ChunkDecoder {
	/// Our reader is a bytes reader that is used to read the bytes.
	/// This is a wrapper around a bytes mut.
	reader: BytesReader,

	/// According to the spec chunk streams are identified by the chunk stream
	/// ID. In this case that is our key.
	/// We then have a chunk header (since some chunks refer to the previous
	/// chunk header)
	previous_chunk_headers: HashMap<u32, ChunkMessageHeader>,

	/// Technically according to the spec, we can have multiple message_streams
	/// in a single chunk_stream Because of this we actually have to have a map
	/// of chunk streams to message streams to bytes The first u32 is the chunk
	/// stream id, the second is the message stream id
	partial_chunks: HashMap<(u32, u32), BytesMut>,

	/// This is the max chunk size that the client has specified.
	/// By default this is 128 bytes.
	max_chunk_size: usize,
}

impl Default for ChunkDecoder {
	fn default() -> Self {
		Self {
			reader: BytesReader::new(BytesMut::new()),
			previous_chunk_headers: HashMap::new(),
			partial_chunks: HashMap::new(),
			max_chunk_size: INIT_CHUNK_SIZE,
		}
	}
}

impl ChunkDecoder {
	/// This function is used to extend the data that we have.f
	pub fn extend_data(&mut self, data: &[u8]) {
		self.reader.extend_from_slice(data);
	}

	/// Sometimes a client will request a chunk size change.
	pub fn update_max_chunk_size(&mut self, chunk_size: usize) -> bool {
		// We need to make sure that the chunk size is within the allowed range.
		// Returning false here will close the connection.
		if !(INIT_CHUNK_SIZE..=MAX_CHUNK_SIZE).contains(&chunk_size) {
			false
		} else {
			self.max_chunk_size = chunk_size;
			true
		}
	}

	/// This function is used to read a chunk from the buffer.
	/// - will return Ok(None) if the buffer is empty.
	/// - will return Ok(Some(Chunk)) if we have a full chunk.
	/// - Err(UnpackError) if we have an error. This will close the connection.
	pub fn read_chunk(&mut self) -> Result<Option<Chunk>, ChunkDecodeError> {
		// We do this in a loop because we may have multiple chunks in the buffer,
		// And those chunks may be partial chunks thus we need to keep reading until we
		// have a full chunk or we run out of data.
		loop {
			// The cursor is an advanced cursor that is a reference to the buffer.
			// This means the cursor does not advance the reader's position.
			// Thus allowing us to backtrack if we need to read more data.
			let mut cursor = self.reader.advance_bytes_cursor(self.reader.len())?;

			let header = match self.read_header(&mut cursor) {
				Ok(header) => header,
				Err(None) => {
					// Returning none here means that the buffer is empty and we need to wait for
					// more data.
					return Ok(None);
				}
				Err(Some(err)) => {
					// This is an error that we can't recover from, so we return it.
					// The connection will be closed.
					return Err(err);
				}
			};

			let message_header = match self.read_message_header(&header, &mut cursor) {
				Ok(message_header) => message_header,
				Err(None) => {
					// Returning none here means that the buffer is empty and we need to wait for
					// more data.
					return Ok(None);
				}
				Err(Some(err)) => {
					// This is an error that we can't recover from, so we return it.
					// The connection will be closed.
					return Err(err);
				}
			};

			let (payload_range_start, payload_range_end) =
				match self.get_payload_range(&header, &message_header, &mut cursor) {
					Ok(data) => data,
					Err(None) => {
						// Returning none here means that the buffer is empty and we need to wait
						// for more data.
						return Ok(None);
					}
					Err(Some(err)) => {
						// This is an error that we can't recover from, so we return it.
						// The connection will be closed.
						return Err(err);
					}
				};

			// Since we were reading from an advanced cursor, our reads did not actually
			// advance the reader's position. We need to manually advance the reader's
			// position to the cursor's position.
			let Ok(data) = self.reader.read_bytes(cursor.position() as usize) else {
				// This means that the payload range was larger than the buffer.
				// This happens when we dont have enough data to read the payload.
				// We need to wait for more data.
				return Ok(None);
			};

			// We freeze the chunk data and slice it to get the payload.
			// Data before the slice is the header data, and data after the slice is the
			// next chunk We don't need to keep the header data, because we already decoded
			// it into struct form. The payload_range_end should be the same as the cursor's
			// position.
			let payload = data.freeze().slice(payload_range_start..payload_range_end);

			// We need to check here if the chunk header is already stored in our map.
			// This isnt a spec check but it is a check to make sure that we dont have too
			// many previous chunk headers stored in memory.
			let count = if self.previous_chunk_headers.contains_key(&header.chunk_stream_id) {
				self.previous_chunk_headers.len()
			} else {
				self.previous_chunk_headers.len() + 1
			};

			// If this is hit, then we have too many previous chunk headers stored in
			// memory. And the client is probably trying to DoS us.
			// We return an error and the connection will be closed.
			if count > MAX_PREVIOUS_CHUNK_HEADERS {
				return Err(ChunkDecodeError::TooManyPreviousChunkHeaders);
			}

			// We insert the chunk header into our map.
			self.previous_chunk_headers
				.insert(header.chunk_stream_id, message_header.clone());

			// It is possible in theory to get a chunk message that requires us to change
			// the max chunk size. However the size of that message is smaller than the
			// default max chunk size. Therefore we can ignore this case.
			// Since if we get such a message we will read it and the payload.len() will be
			// equal to the message length. and thus we will return the chunk.

			// Check if the payload is the same as the message length.
			// If this is true we have a full chunk and we can return it.
			if payload.len() == message_header.msg_length as usize {
				return Ok(Some(Chunk {
					basic_header: header,
					message_header,
					payload,
				}));
			} else {
				// Otherwise we generate a key using the chunk stream id and the message stream
				// id. We then get the partial chunk from the map using the key.
				let key = (header.chunk_stream_id, message_header.msg_stream_id);
				let partial_chunk = match self.partial_chunks.get_mut(&key) {
					Some(partial_chunk) => partial_chunk,
					None => {
						// If it does not exists we create a new one.
						// If we have too many partial chunks we return an error.
						// Since the client is probably trying to DoS us.
						// The connection will be closed.
						if self.partial_chunks.len() >= MAX_PARTIAL_CHUNK_COUNT {
							return Err(ChunkDecodeError::TooManyPartialChunks);
						}

						// Insert a new empty BytesMut into the map.
						self.partial_chunks.insert(key, BytesMut::new());
						// Get the partial chunk we just inserted.
						self.partial_chunks.get_mut(&key).expect("we just inserted it")
					}
				};

				// We extend the partial chunk with the payload.
				// And get the new length of the partial chunk.
				let length = {
					// If the length of a single chunk is larger than the max partial chunk size
					// we return an error. The client is probably trying to DoS us.
					if partial_chunk.len() + payload.len() > MAX_PARTIAL_CHUNK_SIZE {
						return Err(ChunkDecodeError::PartialChunkTooLarge(partial_chunk.len() + payload.len()));
					}

					// Extend the partial chunk with the payload.
					partial_chunk.extend_from_slice(&payload[..]);

					// Return the new length of the partial chunk.
					partial_chunk.len()
				};

				// If we have a full chunk we return it.
				if length == message_header.msg_length as usize {
					return Ok(Some(Chunk {
						basic_header: header,
						message_header,
						payload: self.partial_chunks.remove(&key).unwrap().freeze(),
					}));
				}

				// If we don't have a full chunk we just let the loop continue.
				// Usually this will result in returning Ok(None) from one of
				// the above checks. However there is a edge case that we have
				// enough data in our buffer to read the next chunk and the
				// client is waiting for us to send a response. Meaning if we
				// just return Ok(None) here We would deadlock the connection,
				// and it will eventually timeout. So we need to loop again here
				// to check if we have enough data to read the next chunk.
			}
		}
	}

	/// Internal function used to read the basic chunk header.
	fn read_header(&self, cursor: &mut Cursor<&'_ [u8]>) -> Result<ChunkBasicHeader, Option<ChunkDecodeError>> {
		// The first byte of the basic header is the format of the chunk and the stream
		// id. Mapping the error to none means that this isn't a real error but we dont
		// have enough data.
		let byte = cursor.read_u8().map_err(|_| None)?;
		// The format is the first 2 bits of the byte. We shift the byte 6 bits to the
		// right to get the format.
		let format = (byte >> 6) & 0b00000011;

		// We check that the format is valid.
		// Since we do not map to None here, this is a real error and the connection
		// will be closed. It should not be possible to get an invalid chunk type
		// because, we bitshift the byte 6 bits to the right. Leaving 2 bits which can
		// only be 0, 1 or 2 or 3 which is the only valid chunk types.
		let format = ChunkType::from_u8(format).ok_or(ChunkDecodeError::InvalidChunkType(format))?;

		// We then check the chunk stream id.
		let chunk_stream_id = match (byte & 0b00111111) as u32 {
			// If the chunk stream id is 0 we read the next byte and add 64 to it.
			0 => 64 + cursor.read_u8().map_err(|_| None)? as u32,
			// If it is 1 we read the next 2 bytes and add 64 to it and multiply the 2nd byte by
			// 256.
			1 => 64 + cursor.read_u8().map_err(|_| None)? as u32 + cursor.read_u8().map_err(|_| None)? as u32 * 256,
			// Any other value means that the chunk stream id is the value of the byte.
			csid => csid,
		};

		// We then read the message header.
		let header = ChunkBasicHeader { chunk_stream_id, format };

		Ok(header)
	}

	/// Internal function used to read the message header.
	fn read_message_header(
		&self,
		header: &ChunkBasicHeader,
		cursor: &mut Cursor<&'_ [u8]>,
	) -> Result<ChunkMessageHeader, Option<ChunkDecodeError>> {
		// Each format has a different message header length.
		match header.format {
			// Type0 headers have the most information and can be compared to keyframes in video.
			// They do not reference any previous chunks. They contain the full message header.
			ChunkType::Type0 => {
				// The first 3 bytes are the timestamp.
				let timestamp = cursor.read_u24::<BigEndian>().map_err(|_| None)?;
				// Followed by a 3 byte message length. (this is the length of the entire
				// payload not just this chunk)
				let msg_length = cursor.read_u24::<BigEndian>().map_err(|_| None)?;
				if msg_length as usize > MAX_PARTIAL_CHUNK_SIZE {
					return Err(Some(ChunkDecodeError::PartialChunkTooLarge(msg_length as usize)));
				}

				// We then have a 1 byte message type id.
				let msg_type_id = cursor.read_u8().map_err(|_| None)?;

				// We validate the message type id. If it is invalid we return an error. (this
				// is a real error)
				let msg_type_id =
					MessageTypeID::from_u8(msg_type_id).ok_or(ChunkDecodeError::InvalidMessageTypeID(msg_type_id))?;

				// We then read the message stream id. (According to spec this is stored in
				// LittleEndian, no idea why.)
				let msg_stream_id = cursor.read_u32::<LittleEndian>().map_err(|_| None)?;

				// Sometimes the timestamp is larger than 3 bytes.
				// If the timestamp is 0xFFFFFF we read the next 4 bytes as the timestamp.
				// I am not exactly sure why they did it this way.
				// Why not just use 3 bytes for the timestamp, and if the 3 bytes are set to
				// 0xFFFFFF just read 1 additional byte and then shift it 24 bits.
				// Like if timestamp == 0xFFFFFF { timestamp |= cursor.read_u8().map_err(|_|
				// None)? << 24; } This would save 3 bytes in the header and would be more
				// efficient but I guess the Spec writers are smarter than me.
				let (timestamp, was_extended_timestamp) = if timestamp == 0xFFFFFF {
					// Again this is not a real error, we just dont have enough data.
					(cursor.read_u32::<BigEndian>().map_err(|_| None)?, true)
				} else {
					(timestamp, false)
				};

				Ok(ChunkMessageHeader {
					timestamp,
					msg_length,
					msg_type_id,
					msg_stream_id,
					was_extended_timestamp,
				})
			}
			// For ChunkType 1 we have a delta timestamp, message length and message type id.
			// The message stream id is the same as the previous chunk.
			ChunkType::Type1 => {
				// The first 3 bytes are the delta timestamp.
				let timestamp_delta = cursor.read_u24::<BigEndian>().map_err(|_| None)?;
				// Followed by a 3 byte message length. (this is the length of the entire
				// payload not just this chunk)
				let msg_length = cursor.read_u24::<BigEndian>().map_err(|_| None)?;
				if msg_length as usize > MAX_PARTIAL_CHUNK_SIZE {
					return Err(Some(ChunkDecodeError::PartialChunkTooLarge(msg_length as usize)));
				}

				// We then have a 1 byte message type id.
				let msg_type_id = cursor.read_u8().map_err(|_| None)?;

				// We validate the message type id. If it is invalid we return an error. (this
				// is a real error)
				let msg_type_id =
					MessageTypeID::from_u8(msg_type_id).ok_or(ChunkDecodeError::InvalidMessageTypeID(msg_type_id))?;

				// Again as mentioned above we sometimes have a delta timestamp larger than 3
				// bytes.
				let (timestamp_delta, was_extended_timestamp) = if timestamp_delta == 0xFFFFFF {
					(cursor.read_u32::<BigEndian>().map_err(|_| None)?, true)
				} else {
					(timestamp_delta, false)
				};

				// We get the previous chunk header.
				// If the previous chunk header is not found we return an error. (this is a real
				// error)
				let previous_header = self
					.previous_chunk_headers
					.get(&header.chunk_stream_id)
					.ok_or(ChunkDecodeError::MissingPreviousChunkHeader(header.chunk_stream_id))?;

				// We calculate the timestamp by adding the delta timestamp to the previous
				// timestamp. We need to make sure this does not overflow.
				let timestamp = previous_header.timestamp.checked_add(timestamp_delta).unwrap_or_else(|| {
					tracing::warn!(
						"Timestamp overflow detected. Previous timestamp: {}, delta timestamp: {}, using previous timestamp.",
						previous_header.timestamp,
						timestamp_delta
					);

					previous_header.timestamp
				});

				Ok(ChunkMessageHeader {
					timestamp,
					msg_length,
					msg_type_id,
					was_extended_timestamp,
					// The message stream id is the same as the previous chunk.
					msg_stream_id: previous_header.msg_stream_id,
				})
			}
			// ChunkType2 headers only have a delta timestamp.
			// The message length, message type id and message stream id are the same as the
			// previous chunk.
			ChunkType::Type2 => {
				// We read the delta timestamp.
				let timestamp_delta = cursor.read_u24::<BigEndian>().map_err(|_| None)?;

				// Again if the delta timestamp is larger than 3 bytes we read the next 4 bytes
				// as the timestamp.
				let (timestamp_delta, was_extended_timestamp) = if timestamp_delta == 0xFFFFFF {
					(cursor.read_u32::<BigEndian>().map_err(|_| None)?, true)
				} else {
					(timestamp_delta, false)
				};

				// We get the previous chunk header.
				// If the previous chunk header is not found we return an error. (this is a real
				// error)
				let previous_header = self
					.previous_chunk_headers
					.get(&header.chunk_stream_id)
					.ok_or(ChunkDecodeError::MissingPreviousChunkHeader(header.chunk_stream_id))?;

				// We calculate the timestamp by adding the delta timestamp to the previous
				// timestamp.
				let timestamp = previous_header.timestamp + timestamp_delta;

				Ok(ChunkMessageHeader {
					timestamp,
					msg_length: previous_header.msg_length,
					msg_type_id: previous_header.msg_type_id,
					msg_stream_id: previous_header.msg_stream_id,
					was_extended_timestamp,
				})
			}
			// ChunkType3 headers are the same as the previous chunk header.
			ChunkType::Type3 => {
				// We get the previous chunk header.
				// If the previous chunk header is not found we return an error. (this is a real
				// error)
				let previous_header = self
					.previous_chunk_headers
					.get(&header.chunk_stream_id)
					.ok_or(ChunkDecodeError::MissingPreviousChunkHeader(header.chunk_stream_id))?
					.clone();

				// Now this is truely stupid.
				// If the PREVIOUS HEADER is extended then we now waste an additional 4 bytes to
				// read the timestamp. Why not just read the timestamp in the previous header if
				// it is extended? I guess the spec writers had some reason and its obviously
				// way above my knowledge.
				if previous_header.was_extended_timestamp {
					// Not a real error, we just dont have enough data.
					// We dont have to store this value since it is the same as the previous header.
					cursor.read_u32::<BigEndian>().map_err(|_| None)?;
				}

				Ok(previous_header)
			}
		}
	}

	/// Internal function to get the payload range of a chunk.
	fn get_payload_range(
		&self,
		header: &ChunkBasicHeader,
		message_header: &ChunkMessageHeader,
		cursor: &mut Cursor<&'_ [u8]>,
	) -> Result<(usize, usize), Option<ChunkDecodeError>> {
		// We find out if the chunk is a partial chunk (and if we have already read some
		// of it).
		let key = (header.chunk_stream_id, message_header.msg_stream_id);

		// Check how much we still need to read (if we have already read some of the
		// chunk)
		let remaining_read_length =
			message_header.msg_length as usize - self.partial_chunks.get(&key).map(|data| data.len()).unwrap_or(0);

		// We get the min between our max chunk size and the remaining read length.
		// This is the amount of bytes we need to read.
		let need_read_length = min(remaining_read_length, self.max_chunk_size);

		// We get the current position in the cursor.
		let pos = cursor.position() as usize;

		// We seek forward to where the payload starts.
		cursor.seek(SeekFrom::Current(need_read_length as i64)).map_err(|_| None)?;

		// We then return the range of the payload.
		// Which would be the pos to the pos + need_read_length.
		Ok((pos, pos + need_read_length))
	}
}
