#[derive(Debug, thiserror::Error)]
pub enum HandshakeError {
    #[error("digest error: {0}")]
    Digest(#[from] DigestError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum DigestError {
    #[error("not enough data")]
    NotEnoughData,
    #[error("digest length not correct")]
    DigestLengthNotCorrect,
    #[error("cannot generate digest")]
    CannotGenerate,
    #[error("unknown schema")]
    UnknownSchema,
}

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use byteorder::ReadBytesExt;

    use super::*;

    #[test]
    fn test_error_display() {
        let err = HandshakeError::Digest(DigestError::CannotGenerate);
        assert_eq!(err.to_string(), "digest error: cannot generate digest");

        let err = HandshakeError::Digest(DigestError::DigestLengthNotCorrect);
        assert_eq!(err.to_string(), "digest error: digest length not correct");

        let err = HandshakeError::Digest(DigestError::UnknownSchema);
        assert_eq!(err.to_string(), "digest error: unknown schema");

        let err = HandshakeError::Digest(DigestError::NotEnoughData);
        assert_eq!(err.to_string(), "digest error: not enough data");

        let err = HandshakeError::Io(std::io::Cursor::new(Vec::<u8>::new()).read_u8().unwrap_err());
        assert_eq!(err.to_string(), "io error: failed to fill whole buffer");
    }
}
