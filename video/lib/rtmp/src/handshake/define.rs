use num_derive::FromPrimitive;

/// The schema version.
/// For the complex handshake the schema is either 0 or 1.
/// A chunk is 764 bytes. (1536 - 8) / 2 = 764
/// A schema of 0 means the digest is after the key, thus the digest is at
/// offset 776 bytes (768 + 8). A schema of 1 means the digest is before the key
/// thus the offset is at offset 8 bytes (0 + 8). Where 8 bytes is the time and
/// version. (4 bytes each) The schema is determined by the client.
/// The server will always use the schema the client uses.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SchemaVersion {
	Schema0,
	Schema1,
}

/// The RTMP version.
/// We only support version 3.
#[derive(Copy, Clone, PartialEq, Eq, FromPrimitive)]
#[repr(u8)]
pub enum RtmpVersion {
	Unknown = 0x0,
	Version3 = 0x3,
}

/// The state of the handshake.
/// This is used to determine what the next step is.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ServerHandshakeState {
	ReadC0C1,
	WriteS0S1S2,
	ReadC2,
	Finish,
}

/// This is the total size of the C1/S1 C2/S2 packets.
pub const RTMP_HANDSHAKE_SIZE: usize = 1536;

/// This is some magic number, I do not know why its 0x04050001 however, the
/// reference implementation uses this value. https://blog.csdn.net/win_lin/article/details/13006803
pub const RTMP_SERVER_VERSION: u32 = 0x04050001;

/// This is the length of the digest.
/// There is a lot of random data before and after the digest, however, the
/// digest is always 32 bytes.
pub const RTMP_DIGEST_LENGTH: usize = 32;

/// This is the length of the time and version.
/// The time is 4 bytes and the version is 4 bytes.
pub const TIME_VERSION_LENGTH: usize = 8;

/// This is the length of the chunk.
/// The chunk is 764 bytes. or (1536 - 8) / 2 = 764
pub const CHUNK_LENGTH: usize = (RTMP_HANDSHAKE_SIZE - TIME_VERSION_LENGTH) / 2;

/// This is the first half of the server key.
/// Defined https://blog.csdn.net/win_lin/article/details/13006803
pub const RTMP_SERVER_KEY_FIRST_HALF: &str = "Genuine Adobe Flash Media Server 001";

/// This is the first half of the client key.
/// Defined https://blog.csdn.net/win_lin/article/details/13006803
pub const RTMP_CLIENT_KEY_FIRST_HALF: &str = "Genuine Adobe Flash Player 001";

/// This is the second half of the server/client key.
/// Used for the complex handshake.
/// Defined https://blog.csdn.net/win_lin/article/details/13006803
pub const RTMP_SERVER_KEY: [u8; 68] = [
	0x47, 0x65, 0x6e, 0x75, 0x69, 0x6e, 0x65, 0x20, 0x41, 0x64, 0x6f, 0x62, 0x65, 0x20, 0x46, 0x6c, 0x61, 0x73, 0x68, 0x20,
	0x4d, 0x65, 0x64, 0x69, 0x61, 0x20, 0x53, 0x65, 0x72, 0x76, 0x65, 0x72, 0x20, 0x30, 0x30, 0x31, 0xf0, 0xee, 0xc2, 0x4a,
	0x80, 0x68, 0xbe, 0xe8, 0x2e, 0x00, 0xd0, 0xd1, 0x02, 0x9e, 0x7e, 0x57, 0x6e, 0xec, 0x5d, 0x2d, 0x29, 0x80, 0x6f, 0xab,
	0x93, 0xb8, 0xe6, 0x36, 0xcf, 0xeb, 0x31, 0xae,
];
