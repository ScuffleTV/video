use std::fmt;

use amf0::Amf0ReadError;

use crate::macros::from_error;
use crate::protocol_control_messages::ProtocolControlMessageError;

#[derive(Debug)]
pub enum MessageError {
	Amf0Read(Amf0ReadError),
	ProtocolControlMessage(ProtocolControlMessageError),
}

from_error!(MessageError, Self::Amf0Read, Amf0ReadError);
from_error!(MessageError, Self::ProtocolControlMessage, ProtocolControlMessageError);

impl fmt::Display for MessageError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match &self {
			Self::Amf0Read(error) => write!(f, "amf0 read error: {}", error),
			Self::ProtocolControlMessage(error) => {
				write!(f, "protocol control message error: {}", error)
			}
		}
	}
}
