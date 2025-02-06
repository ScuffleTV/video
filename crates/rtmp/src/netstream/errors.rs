use scuffle_amf0::Amf0WriteError;

use crate::chunk::ChunkEncodeError;

#[derive(Debug, thiserror::Error)]
pub enum NetStreamError {
    #[error("amf0 write error: {0}")]
    Amf0Write(#[from] Amf0WriteError),
    #[error("chunk encode error: {0}")]
    ChunkEncode(#[from] ChunkEncodeError),
}

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let error = NetStreamError::Amf0Write(Amf0WriteError::NormalStringTooLong);
        assert_eq!(error.to_string(), "amf0 write error: normal string too long");

        let error = NetStreamError::ChunkEncode(ChunkEncodeError::UnknownReadState);
        assert_eq!(error.to_string(), "chunk encode error: unknown read state");
    }
}
