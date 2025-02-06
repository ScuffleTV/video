use std::io;

use crate::chunk::ChunkEncodeError;

#[derive(Debug, thiserror::Error)]
pub enum ProtocolControlMessageError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("chunk encode error: {0}")]
    ChunkEncode(#[from] ChunkEncodeError),
}

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let error = ProtocolControlMessageError::ChunkEncode(ChunkEncodeError::UnknownReadState);
        assert_eq!(error.to_string(), "chunk encode error: unknown read state");

        let error = ProtocolControlMessageError::Io(std::io::Error::from(std::io::ErrorKind::Other));
        assert_eq!(error.to_string(), "io error: other error");
    }
}
