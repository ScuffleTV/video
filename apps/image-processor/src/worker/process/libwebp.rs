#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum WebPError {
	#[error("unknown error {0}")]
	UnknownError(&'static str),
	#[error("out of memory")]
	OutOfMemory,
	#[error("invalid data")]
	InvalidData,
}

pub fn zero_memory_default<T>() -> T {
	unsafe { std::mem::zeroed() }
}
