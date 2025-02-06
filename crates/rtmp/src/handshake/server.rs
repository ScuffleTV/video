use std::io::{self, Seek, Write};
use std::time::SystemTime;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::{BufMut, Bytes, BytesMut};
use rand::Rng;
use scuffle_bytes_util::BytesCursorExt;

use super::define::{self, RtmpVersion, SchemaVersion, ServerHandshakeState};
use super::digest::DigestProcessor;
use super::errors::HandshakeError;

// Simple Handshake Server
// RTMP Spec 1.0 - 5.2
pub struct SimpleHandshakeServer {
    version: RtmpVersion,
    requested_version: RtmpVersion,

    state: ServerHandshakeState,

    c1_bytes: Bytes,
    c1_timestamp: u32,
}

impl Default for SimpleHandshakeServer {
    fn default() -> Self {
        Self {
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

    state: ServerHandshakeState,
    schema_version: SchemaVersion,

    c1_digest: Bytes,
    c1_timestamp: u32,
    c1_version: u32,
}

impl Default for ComplexHandshakeServer {
    fn default() -> Self {
        Self {
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
    /// Perform the handshake, writing to the output and reading from the input.
    pub fn handshake(&mut self, input: &mut io::Cursor<Bytes>, output: &mut Vec<u8>) -> Result<(), HandshakeError> {
        match self.state {
            ServerHandshakeState::ReadC0C1 => {
                self.read_c0(input)?;
                self.read_c1(input)?;
                self.write_s0(output)?;
                self.write_s1(output)?;
                self.write_s2(output)?;
                self.state = ServerHandshakeState::ReadC2;
            }
            ServerHandshakeState::ReadC2 => {
                self.read_c2(input)?;
                self.state = ServerHandshakeState::Finish;
            }
            ServerHandshakeState::Finish => {}
        }

        Ok(())
    }

    fn read_c0(&mut self, input: &mut io::Cursor<Bytes>) -> Result<(), HandshakeError> {
        // Version (8 bits): In C0, this field identifies the RTMP version
        //  requested by the client.
        let requested_version = input.read_u8()?;
        self.requested_version = match requested_version {
            3 => RtmpVersion::Version3,
            _ => RtmpVersion::Unknown,
        };

        // We only support version 3 for now.
        // Therefore we set the version to 3.
        self.version = RtmpVersion::Version3;

        Ok(())
    }

    fn read_c1(&mut self, input: &mut io::Cursor<Bytes>) -> Result<(), HandshakeError> {
        // Time (4 bytes): This field contains a timestamp, which SHOULD be
        //  used as the epoch for all future chunks sent from this endpoint.
        //  This may be 0, or some arbitrary value. To synchronize multiple
        //  chunkstreams, the endpoint may wish to send the current value of
        //  the other chunkstream’s timestamp.
        self.c1_timestamp = input.read_u32::<BigEndian>()?;

        // Zero (4 bytes): This field MUST be all 0s.
        input.read_u32::<BigEndian>()?;

        // Random data (1528 bytes): This field can contain any arbitrary
        //  values. Since each endpoint has to distinguish between the
        //  response to the handshake it has initiated and the handshake
        //  initiated by its peer,this data SHOULD send something sufficiently
        //  random. But there is no need for cryptographically-secure
        //  randomness, or even dynamic values.
        self.c1_bytes = input.extract_bytes(define::RTMP_HANDSHAKE_SIZE - define::TIME_VERSION_LENGTH)?;

        Ok(())
    }

    fn read_c2(&mut self, input: &mut io::Cursor<Bytes>) -> Result<(), HandshakeError> {
        // We don't care too much about the data in C2, so we just read it
        //  and discard it.
        // We should technically check that the timestamp is the same as
        //  the one we sent in S1, but we don't care. And that the random
        //  data is the same as the one we sent in S2, but we don't care.
        //  Some clients are not strict to spec and send different data.
        // We can just ignore it and not be super strict.
        input.seek_relative(define::RTMP_HANDSHAKE_SIZE as i64)?;

        Ok(())
    }

    /// Defined in RTMP Specification 1.0 - 5.2.2
    fn write_s0(&mut self, output: &mut Vec<u8>) -> Result<(), HandshakeError> {
        // Version (8 bits): In S0, this field identifies the RTMP
        //  version selected by the server. The version defined by this
        //  specification is 3. A server that does not recognize the
        //  client’s requested version SHOULD respond with 3. The client MAY
        //  choose to degrade to version 3, or to abandon the handshake.
        output.write_u8(self.version as u8)?;

        Ok(())
    }

    /// Defined in RTMP Specification 1.0 - 5.2.3
    fn write_s1(&mut self, output: &mut Vec<u8>) -> Result<(), HandshakeError> {
        // Time (4 bytes): This field contains a timestamp, which SHOULD be
        //  used as the epoch for all future chunks sent from this endpoint.
        //  This may be 0, or some arbitrary value. To synchronize multiple
        //  chunkstreams, the endpoint may wish to send the current value of
        //  the other chunkstream’s timestamp.
        output.write_u32::<BigEndian>(current_time())?;

        // Zero(4 bytes): This field MUST be all 0s.
        output.write_u32::<BigEndian>(0)?;

        // Random data (1528 bytes): This field can contain any arbitrary
        //  values. Since each endpoint has to distinguish between the
        //  response to the handshake it has initiated and the handshake
        //  initiated by its peer,this data SHOULD send something sufficiently
        //  random. But there is no need for cryptographically-secure
        //  randomness, or even dynamic values.
        let mut rng = rand::thread_rng();
        for _ in 0..1528 {
            output.write_u8(rng.gen())?;
        }

        Ok(())
    }

    fn write_s2(&mut self, output: &mut Vec<u8>) -> Result<(), HandshakeError> {
        // Time (4 bytes): This field MUST contain the timestamp sent by the C1 (for
        // S2).
        output.write_u32::<BigEndian>(self.c1_timestamp)?;

        // Time2 (4 bytes): This field MUST contain the timestamp at which the
        //  previous packet(s1 or c1) sent by the peer was read.
        output.write_u32::<BigEndian>(current_time())?;

        // Random echo (1528 bytes): This field MUST contain the random data
        //  field sent by the peer in S1 (for C2) or S2 (for C1). Either peer
        //  can use the time and time2 fields together with the current
        //  timestamp as a quick estimate of the bandwidth and/or latency of
        //  the connection, but this is unlikely to be useful.
        output.write_all(&self.c1_bytes[..])?;

        Ok(())
    }
}

impl ComplexHandshakeServer {
    pub fn handshake(&mut self, input: &mut io::Cursor<Bytes>, output: &mut Vec<u8>) -> Result<(), HandshakeError> {
        match self.state {
            ServerHandshakeState::ReadC0C1 => {
                self.read_c0(input)?;
                self.read_c1(input)?;
                self.write_s0(output)?;
                self.write_s1(output)?;
                self.write_s2(output)?;
                self.state = ServerHandshakeState::ReadC2;
            }
            ServerHandshakeState::ReadC2 => {
                self.read_c2(input)?;
                self.state = ServerHandshakeState::Finish;
            }
            ServerHandshakeState::Finish => {}
        }

        Ok(())
    }

    fn read_c0(&mut self, input: &mut io::Cursor<Bytes>) -> Result<(), HandshakeError> {
        // Version (8 bits): In C0, this field identifies the RTMP version
        //  requested by the client.
        let requested_version = input.read_u8()?;
        self.requested_version = match requested_version {
            3 => RtmpVersion::Version3,
            _ => RtmpVersion::Unknown,
        };

        // We only support version 3 for now.
        // Therefore we set the version to 3.
        self.version = RtmpVersion::Version3;

        Ok(())
    }

    fn read_c1(&mut self, input: &mut io::Cursor<Bytes>) -> Result<(), HandshakeError> {
        let c1_bytes = input.extract_bytes(define::RTMP_HANDSHAKE_SIZE)?;

        //  The first 4 bytes of C1 are the timestamp.
        self.c1_timestamp = (&c1_bytes[0..4]).read_u32::<BigEndian>()?;

        // The next 4 bytes are a version number.
        self.c1_version = (&c1_bytes[4..8]).read_u32::<BigEndian>()?;

        // The following 764 bytes are either the digest or the key.
        let data_digest = DigestProcessor::new(c1_bytes, define::RTMP_CLIENT_KEY_FIRST_HALF);

        let (c1_digest_data, schema_version) = data_digest.read_digest()?;

        self.c1_digest = c1_digest_data;
        self.schema_version = schema_version;

        Ok(())
    }

    fn read_c2(&mut self, input: &mut io::Cursor<Bytes>) -> Result<(), HandshakeError> {
        // We don't care too much about the data in C2, so we just read it
        //  and discard it.
        input.seek_relative(define::RTMP_HANDSHAKE_SIZE as i64)?;

        Ok(())
    }

    fn write_s0(&mut self, output: &mut Vec<u8>) -> Result<(), HandshakeError> {
        // The version of the protocol used in the handshake.
        // This server is using version 3 of the protocol.
        output.write_u8(self.version as u8)?; // 8 bits version

        Ok(())
    }

    fn write_s1(&self, output: &mut Vec<u8>) -> Result<(), HandshakeError> {
        let mut writer = BytesMut::new().writer();

        // The first 4 bytes of S1 are the timestamp.
        writer.write_u32::<BigEndian>(current_time())?;

        // The next 4 bytes are a version number.
        writer.write_u32::<BigEndian>(define::RTMP_SERVER_VERSION)?;

        // We then write 1528 bytes of random data. (764 bytes for digest, 764 bytes for
        // key)
        let mut rng = rand::thread_rng();
        for _ in 0..define::RTMP_HANDSHAKE_SIZE - define::TIME_VERSION_LENGTH {
            writer.write_u8(rng.gen())?;
        }

        // The digest is loaded with the data that we just generated.
        let data_digest = DigestProcessor::new(writer.into_inner().freeze(), define::RTMP_SERVER_KEY_FIRST_HALF);

        // We use the same schema version as the client.
        let (first, second, third) = data_digest.generate_and_fill_digest(self.schema_version)?;

        // We then write the parts of the digest to the main writer.
        // Note: this is not a security issue since we do not flush the buffer until we
        // are done  with the handshake.
        output.write_all(&first)?;
        output.write_all(&second)?;
        output.write_all(&third)?;

        Ok(())
    }

    fn write_s2(&self, output: &mut Vec<u8>) -> Result<(), HandshakeError> {
        let start = output.len();

        // We write the current time to the first 4 bytes.
        output.write_u32::<BigEndian>(current_time())?;

        // We write the timestamp from C1 to the next 4 bytes.
        output.write_u32::<BigEndian>(self.c1_timestamp)?;

        // We then write 1528 bytes of random data. (764 bytes for digest, 764 bytes for
        // key)
        let mut rng = rand::thread_rng();

        // define::RTMP_HANDSHAKE_SIZE - define::TIME_VERSION_LENGTH because we already
        // wrote 8 bytes. (timestamp and c1 timestamp)
        for _ in 0..define::RTMP_HANDSHAKE_SIZE - define::RTMP_DIGEST_LENGTH - define::TIME_VERSION_LENGTH {
            output.write_u8(rng.gen())?;
        }

        // The digest is loaded with the data that we just generated.
        // This digest is used to generate the key. (digest of c1)
        let key_digest = DigestProcessor::new(Bytes::new(), define::RTMP_SERVER_KEY);

        // Create a digest of the random data using a key generated from the digest of
        // C1.
        let key = key_digest.make_digest(&self.c1_digest, &[])?;
        let data_digest = DigestProcessor::new(Bytes::new(), &key);

        // We then generate a digest using the key and the random data
        // We then extract the first 1504 bytes of the data.
        // define::RTMP_HANDSHAKE_SIZE - 32 = 1504
        // 32 is the size of the digest. for C2S2
        let digest = data_digest.make_digest(
            &output[start..start + define::RTMP_HANDSHAKE_SIZE - define::RTMP_DIGEST_LENGTH],
            &[],
        )?;

        // Write the random data  to the main writer.
        // Total Write = 1536 bytes (1504 + 32)
        output.write_all(&digest)?; // 32 bytes of digest

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
pub enum HandshakeServer {
    Simple(SimpleHandshakeServer),
    Complex(ComplexHandshakeServer),
}

impl Default for HandshakeServer {
    fn default() -> Self {
        Self::Complex(ComplexHandshakeServer::default())
    }
}

impl HandshakeServer {
    /// Get the state of the handshake.
    pub fn state(&mut self) -> ServerHandshakeState {
        match self {
            HandshakeServer::Simple(handshaker) => handshaker.state,
            HandshakeServer::Complex(handshaker) => handshaker.state,
        }
    }

    /// Perform the handshake.
    pub fn handshake(&mut self, input: &mut io::Cursor<Bytes>, writer: &mut Vec<u8>) -> Result<(), HandshakeError> {
        match self {
            HandshakeServer::Complex(handshaker) => {
                // We need to be able to go back if the handshake isn't complex.
                let position = input.position();

                let result = handshaker.handshake(input, writer);
                if result.is_err() {
                    // Complex handshake failed, switch to simple handshake.
                    let mut simple = SimpleHandshakeServer::default();

                    // We seek back to the position where we started.
                    input.seek(io::SeekFrom::Start(position))?;

                    // We then perform the handshake.
                    simple.handshake(input, writer)?;

                    // We then set the handshake to simple.
                    *self = HandshakeServer::Simple(simple);
                }
            }
            HandshakeServer::Simple(handshaker) => {
                handshaker.handshake(input, writer)?;
            }
        }

        Ok(())
    }
}

pub fn current_time() -> u32 {
    let duration = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH);
    match duration {
        Ok(result) => result.as_nanos() as u32,
        _ => 0,
    }
}

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use std::io::Read;

    use super::*;

    #[test]
    fn test_simple_handshake() {
        let mut handshake_server = HandshakeServer::default();

        let mut c0c1 = Vec::with_capacity(1528 + 8);
        c0c1.write_u8(3).unwrap(); // version
        c0c1.write_u32::<BigEndian>(123).unwrap(); // timestamp
        c0c1.write_u32::<BigEndian>(0).unwrap(); // zero

        for i in 0..1528 {
            c0c1.write_u8((i % 256) as u8).unwrap();
        }

        let c0c1 = Bytes::from(c0c1);

        let mut writer = Vec::new();
        handshake_server
            .handshake(&mut std::io::Cursor::new(c0c1.clone()), &mut writer)
            .unwrap();

        let mut reader = std::io::Cursor::new(writer);
        assert_eq!(reader.read_u8().unwrap(), 3); // version
        let timestamp = reader.read_u32::<BigEndian>().unwrap(); // timestamp
        assert_eq!(reader.read_u32::<BigEndian>().unwrap(), 0); // zero

        let mut server_random = vec![0; 1528];
        reader.read_exact(&mut server_random).unwrap();

        assert_eq!(reader.read_u32::<BigEndian>().unwrap(), 123); // our timestamp
        let timestamp2 = reader.read_u32::<BigEndian>().unwrap(); // server timestamp

        assert!(timestamp2 >= timestamp);

        let mut read_client_random = vec![0; 1528];
        reader.read_exact(&mut read_client_random).unwrap();

        assert_eq!(&c0c1[9..], &read_client_random);

        let mut c2 = Vec::with_capacity(1528 + 8);
        c2.write_u32::<BigEndian>(timestamp).unwrap(); // timestamp
        c2.write_u32::<BigEndian>(124).unwrap(); // our timestamp
        c2.write_all(&server_random).unwrap();

        let mut writer = Vec::new();
        handshake_server
            .handshake(&mut std::io::Cursor::new(Bytes::from(c2)), &mut writer)
            .unwrap();

        assert_eq!(handshake_server.state(), ServerHandshakeState::Finish)
    }

    #[test]
    fn test_complex_handshake() {
        let mut handshake_server = HandshakeServer::default();

        let mut writer = Vec::with_capacity(3073);
        writer.write_u8(3).unwrap(); // version

        let mut c0c1 = Vec::with_capacity(1528 + 8);
        c0c1.write_u32::<BigEndian>(123).unwrap(); // timestamp
        c0c1.write_u32::<BigEndian>(100).unwrap(); // client version

        for i in 0..1528 {
            c0c1.write_u8((i % 256) as u8).unwrap();
        }

        let data_digest = DigestProcessor::new(Bytes::from(c0c1), define::RTMP_CLIENT_KEY_FIRST_HALF);

        let (first, second, third) = data_digest.generate_and_fill_digest(SchemaVersion::Schema1).unwrap();

        writer.extend_from_slice(&first);
        writer.extend_from_slice(&second);
        writer.extend_from_slice(&third);

        let mut bytes = Vec::new();
        handshake_server
            .handshake(&mut std::io::Cursor::new(Bytes::from(writer)), &mut bytes)
            .unwrap();

        let s0 = &bytes[0..1];
        let s1 = &bytes[1..1537];
        let s2 = &bytes[1537..3073];

        assert_eq!(s0[0], 3); // version
        assert_ne!((&s1[..4]).read_u32::<BigEndian>().unwrap(), 0); // timestamp should not be zero
        assert_eq!((&s1[4..8]).read_u32::<BigEndian>().unwrap(), define::RTMP_SERVER_VERSION); // RTMP version

        let data_digest = DigestProcessor::new(Bytes::copy_from_slice(s1), define::RTMP_SERVER_KEY_FIRST_HALF);

        let (digest, schema) = data_digest.read_digest().unwrap();
        assert_eq!(schema, SchemaVersion::Schema1);

        assert_ne!((&s2[..4]).read_u32::<BigEndian>().unwrap(), 0); // timestamp should not be zero
        assert_eq!((&s2[4..8]).read_u32::<BigEndian>().unwrap(), 123); // our timestamp

        let key_digest = DigestProcessor::new(Bytes::new(), define::RTMP_SERVER_KEY);

        let key = key_digest.make_digest(&second, &[]).unwrap();
        let data_digest = DigestProcessor::new(Bytes::new(), &key);

        assert_eq!(data_digest.make_digest(&s2[..1504], &[]).unwrap(), s2[1504..]);

        let key = key_digest.make_digest(&digest, &[]).unwrap();
        let data_digest = DigestProcessor::new(Bytes::new(), &key);

        let mut c2 = Vec::new();
        for i in 0..1528 {
            c2.write_u8((i % 256) as u8).unwrap();
        }

        let digest = data_digest.make_digest(&c2, &[]).unwrap();

        let mut c2 = Vec::with_capacity(1528 + 8);
        c2.write_u32::<BigEndian>(123).unwrap(); // timestamp
        c2.write_u32::<BigEndian>(124).unwrap(); // our timestamp
        c2.write_all(&digest).unwrap();

        let mut writer = Vec::new();
        handshake_server
            .handshake(&mut std::io::Cursor::new(Bytes::from(c2)), &mut writer)
            .unwrap();

        assert_eq!(handshake_server.state(), ServerHandshakeState::Finish)
    }
}
