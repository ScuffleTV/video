use bytesio::bytesio_errors::BytesIOError;

use crate::chunk::{ChunkDecodeError, ChunkEncodeError};
use crate::handshake::{DigestError, HandshakeError};
use crate::messages::MessageError;
use crate::netconnection::NetConnectionError;
use crate::netstream::NetStreamError;
use crate::protocol_control_messages::ProtocolControlMessageError;
use crate::user_control_messages::EventMessagesError;
use crate::{SessionError, UniqueID};

#[test]
fn test_error_display() {
	let error = SessionError::BytesIO(BytesIOError::ClientClosed);
	assert_eq!(error.to_string(), "bytesio error: client closed");

	let error = SessionError::Handshake(HandshakeError::Digest(DigestError::NotEnoughData));
	assert_eq!(error.to_string(), "handshake error: digest error: not enough data");

	let error = SessionError::Message(MessageError::Amf0Read(amf0::Amf0ReadError::WrongType));
	assert_eq!(error.to_string(), "message error: amf0 read error: wrong type");

	let error = SessionError::ChunkDecode(ChunkDecodeError::TooManyPreviousChunkHeaders);
	assert_eq!(error.to_string(), "chunk decode error: too many previous chunk headers");

	let error =
		SessionError::ProtocolControlMessage(ProtocolControlMessageError::ChunkEncode(ChunkEncodeError::UnknownReadState));
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

	let error = SessionError::PublisherDisconnected(UniqueID::nil());
	assert_eq!(
		error.to_string(),
		"publisher disconnected: 00000000-0000-0000-0000-000000000000"
	);

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
