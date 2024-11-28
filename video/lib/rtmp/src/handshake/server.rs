use std::io::Write;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::{Bytes, BytesMut};
use bytesio::bytes_reader::BytesReader;
use bytesio::bytes_writer::BytesWriter;
use rand::Rng;

use super::define::{RtmpVersion, SchemaVersion, ServerHandshakeState};
use super::digest::DigestProcessor;
use super::errors::HandshakeError;
use super::{define, utils};

// Simple Handshake Server
// RTMP Spec 1.0 - 5.2
pub struct SimpleHandshakeServer {
	version: RtmpVersion,
	requested_version: RtmpVersion,

	reader: BytesReader,

	state: ServerHandshakeState,

	c1_bytes: Bytes,
	c1_timestamp: u32,
}

impl Default for SimpleHandshakeServer {
	fn default() -> Self {
		Self {
			reader: BytesReader::new(BytesMut::default()),
			state: ServerHandshakeState::ReadC0C1,
			c1_bytes: Bytes::new(),
			c1_timestamp: 0,
			version: RtmpVersion::Unknown,
			requested_version: RtmpVersion::Unknown,
		}
	}
}

// Complex Handshake Server
// Unfortunately there doesn't seem to be a good spec sheet for this.
// https://blog.csdn.net/win_lin/article/details/13006803 is the best I could find.
pub struct ComplexHandshakeServer {
	version: RtmpVersion,
	requested_version: RtmpVersion,

	reader: BytesReader,

	state: ServerHandshakeState,
	schema_version: SchemaVersion,

	c1_digest: Bytes,
	c1_timestamp: u32,
	c1_version: u32,
}

impl Default for ComplexHandshakeServer {
	fn default() -> Self {
		Self {
			reader: BytesReader::new(BytesMut::default()),
			state: ServerHandshakeState::ReadC0C1,
			c1_digest: Bytes::default(),
			c1_timestamp: 0,
			version: RtmpVersion::Unknown,
			requested_version: RtmpVersion::Unknown,
			c1_version: 0,
			schema_version: SchemaVersion::Schema0,
		}
	}
}

impl SimpleHandshakeServer {
	pub fn extend_data(&mut self, data: &[u8]) {
		self.reader.extend_from_slice(data);
	}

	pub fn handshake(&mut self, writer: &mut BytesWriter) -> Result<(), HandshakeError> {
		loop {
			match self.state {
				ServerHandshakeState::ReadC0C1 => {
					self.read_c0()?;
					self.read_c1()?;
					self.state = ServerHandshakeState::WriteS0S1S2;
				}
				ServerHandshakeState::WriteS0S1S2 => {
					self.write_s0(writer)?;
					self.write_s1(writer)?;
					self.write_s2(writer)?;
					self.state = ServerHandshakeState::ReadC2;
					break;
				}
				ServerHandshakeState::ReadC2 => {
					self.read_c2()?;
					self.state = ServerHandshakeState::Finish;
				}
				ServerHandshakeState::Finish => {
					break;
				}
			}
		}

		Ok(())
	}

	fn read_c0(&mut self) -> Result<(), HandshakeError> {
		// Version (8 bits): In C0, this field identifies the RTMP version
		//  requested by the client.
		let requested_version = self.reader.read_u8()?;
		self.requested_version = match requested_version {
			3 => RtmpVersion::Version3,
			_ => RtmpVersion::Unknown,
		};

		// We only support version 3 for now.
		// Therefore we set the version to 3.
		self.version = RtmpVersion::Version3;

		Ok(())
	}

	fn read_c1(&mut self) -> Result<(), HandshakeError> {
		// Time (4 bytes): This field contains a timestamp, which SHOULD be
		//  used as the epoch for all future chunks sent from this endpoint.
		//  This may be 0, or some arbitrary value. To synchronize multiple
		//  chunkstreams, the endpoint may wish to send the current value of
		//  the other chunkstream’s timestamp.
		self.c1_timestamp = self.reader.read_u32::<BigEndian>()?;

		// Zero (4 bytes): This field MUST be all 0s.
		self.reader.read_u32::<BigEndian>()?;

		// Random data (1528 bytes): This field can contain any arbitrary
		//  values. Since each endpoint has to distinguish between the
		//  response to the handshake it has initiated and the handshake
		//  initiated by its peer,this data SHOULD send something sufficiently
		//  random. But there is no need for cryptographically-secure
		//  randomness, or even dynamic values.
		self.c1_bytes = self.reader.read_bytes(1528)?.freeze();

		Ok(())
	}

	fn read_c2(&mut self) -> Result<(), HandshakeError> {
		// We don't care too much about the data in C2, so we just read it
		//  and discard it.
		// We should technically check that the timestamp is the same as
		//  the one we sent in S1, but we don't care. And that the random
		//  data is the same as the one we sent in S2, but we don't care.
		//  Some clients are not strict to spec and send different data.
		// We can just ignore it and not be super strict.
		self.reader.read_bytes(define::RTMP_HANDSHAKE_SIZE)?;

		Ok(())
	}

	/// Defined in RTMP Specification 1.0 - 5.2.2
	fn write_s0(&self, writer: &mut BytesWriter) -> Result<(), HandshakeError> {
		// Version (8 bits): In S0, this field identifies the RTMP
		//  version selected by the server. The version defined by this
		//  specification is 3. A server that does not recognize the
		//  client’s requested version SHOULD respond with 3. The client MAY
		//  choose to degrade to version 3, or to abandon the handshake.
		writer.write_u8(self.version as u8)?;

		Ok(())
	}

	/// Defined in RTMP Specification 1.0 - 5.2.3
	fn write_s1(&self, writer: &mut BytesWriter) -> Result<(), HandshakeError> {
		// Time (4 bytes): This field contains a timestamp, which SHOULD be
		//  used as the epoch for all future chunks sent from this endpoint.
		//  This may be 0, or some arbitrary value. To synchronize multiple
		//  chunkstreams, the endpoint may wish to send the current value of
		//  the other chunkstream’s timestamp.
		writer.write_u32::<BigEndian>(utils::current_time())?;

		// Zero(4 bytes): This field MUST be all 0s.
		writer.write_u32::<BigEndian>(0)?;

		// Random data (1528 bytes): This field can contain any arbitrary
		//  values. Since each endpoint has to distinguish between the
		//  response to the handshake it has initiated and the handshake
		//  initiated by its peer,this data SHOULD send something sufficiently
		//  random. But there is no need for cryptographically-secure
		//  randomness, or even dynamic values.
		let mut rng = rand::thread_rng();
		for _ in 0..1528 {
			writer.write_u8(rng.gen())?;
		}

		Ok(())
	}

	fn write_s2(&self, writer: &mut BytesWriter) -> Result<(), HandshakeError> {
		// Time (4 bytes): This field MUST contain the timestamp sent by the C1 (for
		// S2).
		writer.write_u32::<BigEndian>(self.c1_timestamp)?;

		// Time2 (4 bytes): This field MUST contain the timestamp at which the
		//  previous packet(s1 or c1) sent by the peer was read.
		writer.write_u32::<BigEndian>(utils::current_time())?;

		// Random echo (1528 bytes): This field MUST contain the random data
		//  field sent by the peer in S1 (for C2) or S2 (for C1). Either peer
		//  can use the time and time2 fields together with the current
		//  timestamp as a quick estimate of the bandwidth and/or latency of
		//  the connection, but this is unlikely to be useful.
		writer.write_all(&self.c1_bytes[..])?;

		Ok(())
	}
}

impl ComplexHandshakeServer {
	pub fn extend_data(&mut self, data: &[u8]) {
		self.reader.extend_from_slice(data);
	}

	pub fn handshake(&mut self, writer: &mut BytesWriter) -> Result<(), HandshakeError> {
		loop {
			match self.state {
				ServerHandshakeState::ReadC0C1 => {
					self.read_c0()?;
					self.read_c1()?;
					self.state = ServerHandshakeState::WriteS0S1S2;
				}
				ServerHandshakeState::WriteS0S1S2 => {
					self.write_s0(writer)?;
					self.write_s1(writer)?;
					self.write_s2(writer)?;
					self.state = ServerHandshakeState::ReadC2;
					break;
				}
				ServerHandshakeState::ReadC2 => {
					self.read_c2()?;
					self.state = ServerHandshakeState::Finish;
				}
				ServerHandshakeState::Finish => {
					break;
				}
			}
		}

		Ok(())
	}

	fn read_c0(&mut self) -> Result<(), HandshakeError> {
		// Version (8 bits): In C0, this field identifies the RTMP version
		//  requested by the client.
		let requested_version = self.reader.read_u8()?;
		self.requested_version = match requested_version {
			3 => RtmpVersion::Version3,
			_ => RtmpVersion::Unknown,
		};

		// We only support version 3 for now.
		// Therefore we set the version to 3.
		self.version = RtmpVersion::Version3;

		Ok(())
	}

	fn read_c1(&mut self) -> Result<(), HandshakeError> {
		let c1_bytes = self.reader.read_bytes(define::RTMP_HANDSHAKE_SIZE)?.freeze();

		//  The first 4 bytes of C1 are the timestamp.
		self.c1_timestamp = (&c1_bytes[0..4]).read_u32::<BigEndian>()?;

		// The next 4 bytes are a version number.
		self.c1_version = (&c1_bytes[4..8]).read_u32::<BigEndian>()?;

		// The following 764 bytes are either the digest or the key.
		let data_digest = DigestProcessor::new(c1_bytes, Bytes::from_static(define::RTMP_CLIENT_KEY_FIRST_HALF.as_bytes()));

		let (c1_digest_data, schema_version) = data_digest.read_digest()?;

		self.c1_digest = c1_digest_data;
		self.schema_version = schema_version;

		Ok(())
	}

	fn read_c2(&mut self) -> Result<(), HandshakeError> {
		// We don't care too much about the data in C2, so we just read it
		//  and discard it.
		self.reader.read_bytes(define::RTMP_HANDSHAKE_SIZE)?;

		Ok(())
	}

	fn write_s0(&self, writer: &mut BytesWriter) -> Result<(), HandshakeError> {
		// The version of the protocol used in the handshake.
		// This server is using version 3 of the protocol.
		writer.write_u8(self.version as u8)?; // 8 bits version

		Ok(())
	}

	fn write_s1(&self, main_writer: &mut BytesWriter) -> Result<(), HandshakeError> {
		let mut writer = BytesWriter::default();
		// The first 4 bytes of S1 are the timestamp.
		writer.write_u32::<BigEndian>(utils::current_time())?;

		// The next 4 bytes are a version number.
		writer.write_u32::<BigEndian>(define::RTMP_SERVER_VERSION)?;

		// We then write 1528 bytes of random data. (764 bytes for digest, 764 bytes for
		// key)
		let mut rng = rand::thread_rng();
		for _ in 0..define::RTMP_HANDSHAKE_SIZE - define::TIME_VERSION_LENGTH {
			writer.write_u8(rng.gen())?;
		}

		// The digest is loaded with the data that we just generated.
		let data_digest = DigestProcessor::new(
			writer.dispose(),
			Bytes::from_static(define::RTMP_SERVER_KEY_FIRST_HALF.as_bytes()),
		);

		// We use the same schema version as the client.
		let (first, second, third) = data_digest.generate_and_fill_digest(self.schema_version)?;

		// We then write the parts of the digest to the main writer.
		// Note: this is not a security issue since we do not flush the buffer until we
		// are done  with the handshake.
		main_writer.write_all(&first)?;
		main_writer.write_all(&second)?;
		main_writer.write_all(&third)?;

		Ok(())
	}

	fn write_s2(&self, main_writer: &mut BytesWriter) -> Result<(), HandshakeError> {
		let mut writer = BytesWriter::default();

		// We write the current time to the first 4 bytes.
		writer.write_u32::<BigEndian>(utils::current_time())?;

		// We write the timestamp from C1 to the next 4 bytes.
		writer.write_u32::<BigEndian>(self.c1_timestamp)?;

		// We then write 1528 bytes of random data. (764 bytes for digest, 764 bytes for
		// key)
		let mut rng = rand::thread_rng();

		// define::RTMP_HANDSHAKE_SIZE - define::TIME_VERSION_LENGTH because we already
		// wrote 8 bytes. (timestamp and c1 timestamp)
		for _ in 0..define::RTMP_HANDSHAKE_SIZE - define::TIME_VERSION_LENGTH {
			writer.write_u8(rng.gen())?;
		}

		// The digest is loaded with the data that we just generated.
		// This digest is used to generate the key. (digest of c1)
		let key_digest = DigestProcessor::new(Bytes::new(), Bytes::from_static(&define::RTMP_SERVER_KEY));

		// We then extract the first 1504 bytes of the data.
		// define::RTMP_HANDSHAKE_SIZE - 32 = 1504
		// 32 is the size of the digest. for C2S2
		let data = &writer.dispose()[..define::RTMP_HANDSHAKE_SIZE - define::RTMP_DIGEST_LENGTH];

		// Create a digest of the random data using a key generated from the digest of
		// C1.
		let data_digest = DigestProcessor::new(Bytes::new(), key_digest.make_digest(&self.c1_digest, &[])?);

		// We then generate a digest using the key and the random data
		let digest = data_digest.make_digest(data, &[])?;

		// Write the random data  to the main writer.
		main_writer.write_all(data)?; // 1504 bytes of random data
		main_writer.write_all(&digest)?; // 32 bytes of digest

		// Total Write = 1536 bytes (1504 + 32)

		Ok(())
	}
}

// Order of messages:
// Client -> C0 -> Server
// Client -> C1 -> Server
// Client <- S0 <- Server
// Client <- S1 <- Server
// Client <- S2 <- Server
// Client -> C2 -> Server
pub struct HandshakeServer {
	simple_handshaker: SimpleHandshakeServer,
	complex_handshaker: ComplexHandshakeServer,
	is_complex: bool,
	saved_data: BytesMut,
}

impl Default for HandshakeServer {
	fn default() -> Self {
		Self {
			simple_handshaker: SimpleHandshakeServer::default(),
			complex_handshaker: ComplexHandshakeServer::default(),
			// We attempt to do a complex handshake by default. If the client does not support it,
			// we fallback to simple.
			is_complex: true,
			saved_data: BytesMut::default(),
		}
	}
}

impl HandshakeServer {
	pub fn extend_data(&mut self, data: &[u8]) {
		if self.is_complex {
			self.complex_handshaker.extend_data(data);

			// We same the data in case we need to switch to simple handshake.
			self.saved_data.extend_from_slice(data);
		} else {
			self.simple_handshaker.extend_data(data);
		}
	}

	pub fn state(&mut self) -> ServerHandshakeState {
		if self.is_complex {
			self.complex_handshaker.state
		} else {
			self.simple_handshaker.state
		}
	}

	pub fn extract_remaining_bytes(&mut self) -> BytesMut {
		if self.is_complex {
			self.complex_handshaker.reader.extract_remaining_bytes()
		} else {
			self.simple_handshaker.reader.extract_remaining_bytes()
		}
	}

	pub fn handshake(&mut self, writer: &mut BytesWriter) -> Result<(), HandshakeError> {
		if self.is_complex {
			let result = self.complex_handshaker.handshake(writer);
			if result.is_err() {
				// Complex handshake failed, switch to simple handshake.
				self.is_complex = false;

				// Get the data that was saved in case we need to switch to simple handshake.
				let data = self.saved_data.clone();

				// We then extend the data to the simple handshaker.
				self.extend_data(&data[..]);

				// We then perform the handshake.
				self.simple_handshaker.handshake(writer)?;
			}
		} else {
			self.simple_handshaker.handshake(writer)?;
		}

		Ok(())
	}
}
