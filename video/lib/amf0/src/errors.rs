use std::{fmt, io, str};

use super::define::Amf0Marker;
use super::Amf0Value;

#[derive(Debug)]
pub enum Amf0ReadError {
	UnknownMarker(u8),
	UnsupportedType(Amf0Marker),
	StringParseError(str::Utf8Error),
	IO(io::Error),
	WrongType,
}

macro_rules! from_error {
	($tt:ty, $val:expr, $err:ty) => {
		impl From<$err> for $tt {
			fn from(error: $err) -> Self {
				$val(error)
			}
		}
	};
}

from_error!(Amf0ReadError, Self::StringParseError, str::Utf8Error);
from_error!(Amf0ReadError, Self::IO, io::Error);

#[derive(Debug)]
pub enum Amf0WriteError {
	NormalStringTooLong,
	IO(io::Error),
	UnsupportedType(Amf0Value),
}

from_error!(Amf0WriteError, Self::IO, io::Error);

impl fmt::Display for Amf0ReadError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Self::UnknownMarker(marker) => {
				write!(f, "unknown marker: {}", marker)
			}
			Self::UnsupportedType(marker) => {
				write!(f, "unsupported type: {:?}", marker)
			}
			Self::WrongType => write!(f, "wrong type"),
			Self::StringParseError(err) => write!(f, "string parse error: {}", err),
			Self::IO(err) => write!(f, "io error: {}", err),
		}
	}
}

impl fmt::Display for Amf0WriteError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			Self::NormalStringTooLong => {
				write!(f, "normal string too long")
			}
			Self::UnsupportedType(value_type) => {
				write!(f, "unsupported type: {:?}", value_type)
			}
			Self::IO(error) => write!(f, "io error: {}", error),
		}
	}
}
