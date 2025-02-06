use crate::chunk::ChunkEncodeError;

#[derive(Debug, thiserror::Error)]
pub enum EventMessagesError {
    #[error("chunk encode error: {0}")]
    ChunkEncode(#[from] ChunkEncodeError),
}

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let error = EventMessagesError::ChunkEncode(ChunkEncodeError::UnknownReadState);
        assert_eq!(format!("{}", error), "chunk encode error: unknown read state");
    }
}
