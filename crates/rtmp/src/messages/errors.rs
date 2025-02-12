use scuffle_amf0::Amf0ReadError;

use crate::protocol_control_messages::ProtocolControlMessageError;

#[derive(Debug, thiserror::Error)]
pub enum MessageError {
    #[error("amf0 read error: {0}")]
    Amf0Read(#[from] Amf0ReadError),
    #[error("protocol control message error: {0}")]
    ProtocolControlMessage(#[from] ProtocolControlMessageError),
}

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use scuffle_amf0::Amf0Marker;

    use super::*;
    use crate::chunk::ChunkEncodeError;

    #[test]
    fn test_error_display() {
        let error = MessageError::Amf0Read(Amf0ReadError::WrongType(Amf0Marker::String, Amf0Marker::Date));
        assert_eq!(error.to_string(), "amf0 read error: wrong type: expected String, got Date");

        let error = MessageError::ProtocolControlMessage(ProtocolControlMessageError::ChunkEncode(
            ChunkEncodeError::UnknownReadState,
        ));
        assert_eq!(
            error.to_string(),
            "protocol control message error: chunk encode error: unknown read state"
        );
    }
}
