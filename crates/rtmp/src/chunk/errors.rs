use std::io;

#[derive(Debug, thiserror::Error)]
pub enum ChunkDecodeError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("invalid chunk type: {0}")]
    InvalidChunkType(u8),
    #[error("invalid message type id: {0}")]
    InvalidMessageTypeID(u8),
    #[error("missing previous chunk header: {0}")]
    MissingPreviousChunkHeader(u32),
    #[error("too many partial chunks")]
    TooManyPartialChunks,
    #[error("too many previous chunk headers")]
    TooManyPreviousChunkHeaders,
    #[error("partial chunk too large: {0}")]
    PartialChunkTooLarge(usize),
    #[error("timestamp overflow: timestamp: {0}, delta: {1}")]
    TimestampOverflow(u32, u32),
}

#[derive(Debug, thiserror::Error)]
pub enum ChunkEncodeError {
    #[error("unknown read state")]
    UnknownReadState,
    #[error("io error: {0}")]
    Io(#[from] io::Error),
}
