use crate::chunk::ChunkDecodeError;
use crate::handshake::HandshakeError;
use crate::messages::MessageError;
use crate::netconnection::NetConnectionError;
use crate::netstream::NetStreamError;
use crate::protocol_control_messages::ProtocolControlMessageError;
use crate::user_control_messages::EventMessagesError;

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("handshake error: {0}")]
    Handshake(#[from] HandshakeError),
    #[error("message error: {0}")]
    Message(#[from] MessageError),
    #[error("chunk decode error: {0}")]
    ChunkDecode(#[from] ChunkDecodeError),
    #[error("protocol control message error: {0}")]
    ProtocolControlMessage(#[from] ProtocolControlMessageError),
    #[error("netstream error: {0}")]
    NetStream(#[from] NetStreamError),
    #[error("netconnection error: {0}")]
    NetConnection(#[from] NetConnectionError),
    #[error("event messages error: {0}")]
    EventMessages(#[from] EventMessagesError),
    #[error("unknown stream id: {0}")]
    UnknownStreamID(u32),
    #[error("publisher disconnected")]
    PublisherDisconnected,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("timeout: {0}")]
    Timeout(#[from] tokio::time::error::Elapsed),
    #[error("no app name")]
    NoAppName,
    #[error("no stream name")]
    NoStreamName,
    #[error("publish request denied")]
    PublishRequestDenied,
    #[error("connect request denied")]
    ConnectRequestDenied,
    #[error("play not supported")]
    PlayNotSupported,
    #[error("publisher dropped")]
    PublisherDropped,
    #[error("invalid chunk size: {0}")]
    InvalidChunkSize(usize),
}

impl SessionError {
    pub fn is_client_closed(&self) -> bool {
        match self {
            Self::Io(err) => matches!(
                err.kind(),
                std::io::ErrorKind::ConnectionAborted
                    | std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::UnexpectedEof
            ),
            Self::Timeout(_) => true,
            _ => false,
        }
    }
}

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use scuffle_amf0::Amf0Marker;

    use super::*;
    use crate::chunk::ChunkEncodeError;
    use crate::handshake::DigestError;

    #[test]
    fn test_error_display() {
        let error = SessionError::Io(std::io::Error::new(std::io::ErrorKind::ConnectionAborted, "client closed"));
        assert_eq!(error.to_string(), "io error: client closed");

        let error = SessionError::Handshake(HandshakeError::Digest(DigestError::NotEnoughData));
        assert_eq!(error.to_string(), "handshake error: digest error: not enough data");

        let error = SessionError::Message(MessageError::Amf0Read(scuffle_amf0::Amf0ReadError::WrongType(
            Amf0Marker::String,
            Amf0Marker::EcmaArray,
        )));
        assert_eq!(
            error.to_string(),
            "message error: amf0 read error: wrong type: expected String, got EcmaArray"
        );

        let error = SessionError::ChunkDecode(ChunkDecodeError::TooManyPreviousChunkHeaders);
        assert_eq!(error.to_string(), "chunk decode error: too many previous chunk headers");

        let error = SessionError::ProtocolControlMessage(ProtocolControlMessageError::ChunkEncode(
            ChunkEncodeError::UnknownReadState,
        ));
        assert_eq!(
            error.to_string(),
            "protocol control message error: chunk encode error: unknown read state"
        );

        let error = SessionError::NetStream(NetStreamError::ChunkEncode(ChunkEncodeError::UnknownReadState));
        assert_eq!(error.to_string(), "netstream error: chunk encode error: unknown read state");

        let error = SessionError::NetConnection(NetConnectionError::ChunkEncode(ChunkEncodeError::UnknownReadState));
        assert_eq!(
            error.to_string(),
            "netconnection error: chunk encode error: unknown read state"
        );

        let error = SessionError::EventMessages(EventMessagesError::ChunkEncode(ChunkEncodeError::UnknownReadState));
        assert_eq!(
            error.to_string(),
            "event messages error: chunk encode error: unknown read state"
        );

        let error = SessionError::UnknownStreamID(0);
        assert_eq!(error.to_string(), "unknown stream id: 0");

        let error = SessionError::PublisherDisconnected;
        assert_eq!(error.to_string(), "publisher disconnected");

        let error = SessionError::NoAppName;
        assert_eq!(error.to_string(), "no app name");

        let error = SessionError::NoStreamName;
        assert_eq!(error.to_string(), "no stream name");

        let error = SessionError::PublishRequestDenied;
        assert_eq!(error.to_string(), "publish request denied");

        let error = SessionError::ConnectRequestDenied;
        assert_eq!(error.to_string(), "connect request denied");

        let error = SessionError::PlayNotSupported;
        assert_eq!(error.to_string(), "play not supported");

        let error = SessionError::PublisherDropped;
        assert_eq!(error.to_string(), "publisher dropped");

        let error = SessionError::InvalidChunkSize(123);
        assert_eq!(error.to_string(), "invalid chunk size: 123");
    }
}
