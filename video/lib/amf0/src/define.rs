use std::collections::HashMap;

use num_derive::FromPrimitive;

/// AMF0 marker types.
/// Defined in amf0_spec_121207.pdf section 2.1
#[derive(Debug, PartialEq, Eq, Clone, Copy, FromPrimitive)]
#[repr(u8)]
pub enum Amf0Marker {
	Number = 0x00,
	Boolean = 0x01,
	String = 0x02,
	Object = 0x03,
	MovieClipMarker = 0x04, // reserved, not supported
	Null = 0x05,
	Undefined = 0x06,
	Reference = 0x07,
	EcmaArray = 0x08,
	ObjectEnd = 0x09,
	StrictArray = 0x0a,
	Date = 0x0b,
	LongString = 0x0c,
	Unsupported = 0x0d,
	Recordset = 0x0e, // reserved, not supported
	XmlDocument = 0x0f,
	TypedObject = 0x10,
	AVMPlusObject = 0x11, // AMF3 marker
}

#[derive(PartialEq, Clone, Debug)]
pub enum Amf0Value {
	/// Number Type defined section 2.2
	Number(f64),
	/// Boolean Type defined section 2.3
	Boolean(bool),
	/// String Type defined section 2.4
	String(String),
	/// Object Type defined section 2.5
	Object(HashMap<String, Amf0Value>),
	/// Null Type defined section 2.7
	Null,
	/// Undefined Type defined section 2.8
	ObjectEnd,
	/// LongString Type defined section 2.14
	LongString(String),
}
