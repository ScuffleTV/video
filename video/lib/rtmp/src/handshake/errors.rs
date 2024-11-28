use std::fmt;

use crate::macros::from_error;

#[derive(Debug)]
pub enum HandshakeError {
	Digest(DigestError),
	IO(std::io::Error),
}

from_error!(HandshakeError, Self::Digest, DigestError);
from_error!(HandshakeError, Self::IO, std::io::Error);

impl fmt::Display for HandshakeError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Self::Digest(error) => write!(f, "digest error: {}", error),
			Self::IO(error) => write!(f, "io error: {}", error),
		}
	}
}

#[derive(Debug)]
pub enum DigestError {
	NotEnoughData,
	DigestLengthNotCorrect,
	CannotGenerate,
	UnknownSchema,
}

impl fmt::Display for DigestError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Self::NotEnoughData => write!(f, "not enough data"),
			Self::DigestLengthNotCorrect => write!(f, "digest length not correct"),
			Self::CannotGenerate => write!(f, "cannot generate digest"),
			Self::UnknownSchema => write!(f, "unknown schema"),
		}
	}
}
