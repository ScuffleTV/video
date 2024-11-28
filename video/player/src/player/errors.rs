use wasm_bindgen::prelude::*;

use super::fetch::{FetchError, FetchResult};
use super::JsResult;

#[wasm_bindgen(typescript_custom_section)]
const _: &'static str = r#"
interface EventError {
    code: ErrorCode;
    message: string;
    fatal: boolean;
    source?: any;
};

enum ErrorCode {
    MediaError = "SCUFFLE_MEDIA_ERROR",
    NetworkError = "SCUFFLE_NETWORK_ERROR",
    DecodeError = "SCUFFLE_DECODE_ERROR",
    OtherError = "SCUFFLE_OTHER_ERROR",
}
"#;

#[derive(Debug, Clone, serde::Serialize, tsify::Tsify)]
pub struct EventError {
	pub code: ErrorCode,
	#[serde(with = "serde_wasm_bindgen::preserve")]
	pub source: JsValue,
	pub message: String,
	pub fatal: bool,
}

impl EventError {
	pub fn new(code: ErrorCode, message: String, fatal: bool) -> Self {
		Self {
			code,
			source: JsValue::NULL,
			message,
			fatal,
		}
	}

	pub fn with_source(mut self, source: JsValue) -> Self {
		self.source = source;
		self
	}

	pub fn set_fatal(&mut self, fatal: bool) {
		self.fatal = fatal;
	}
}

#[allow(dead_code)]
pub trait EventErrorExt<T>
where
	Self: Sized,
{
	fn network_error(self, message: &'static str, fatal: bool) -> Result<T, EventError> {
		self.convert_error(ErrorCode::Network, message, fatal)
	}
	fn decode_error(self, message: &'static str, fatal: bool) -> Result<T, EventError> {
		self.convert_error(ErrorCode::Decode, message, fatal)
	}
	fn other_error(self, message: &'static str, fatal: bool) -> Result<T, EventError> {
		self.convert_error(ErrorCode::Other, message, fatal)
	}
	fn convert_error(self, code: ErrorCode, message: &'static str, fatal: bool) -> Result<T, EventError>;
}

impl<T> EventErrorExt<T> for JsResult<T> {
	fn convert_error(self, code: ErrorCode, message: &'static str, fatal: bool) -> Result<T, EventError> {
		self.map_err(|e| EventError {
			code,
			message: message.to_string(),
			fatal,
			source: e,
		})
	}
}

pub trait EventErrorExtFetch<T> {
	fn into_event_error(self, fatal: bool) -> Result<T, EventError>;
}

impl<T> EventErrorExtFetch<T> for FetchResult<T> {
	fn into_event_error(self, fatal: bool) -> Result<T, EventError> {
		self.map_err(|err| match err {
			FetchError::Aborted => EventError::new(ErrorCode::Network, "request was aborted".into(), fatal),
			FetchError::EmptyResponse => EventError::new(ErrorCode::Network, "response was empty".into(), fatal),
			FetchError::InvalidResponse => EventError::new(ErrorCode::Network, "response was invalid".into(), fatal),
			FetchError::JsValue(e) => EventError::new(ErrorCode::Network, "Javascript Error".into(), fatal).with_source(e),
			FetchError::Json(e) => EventError::new(ErrorCode::Network, format!("failed to parse json: {}", e), fatal),
			FetchError::StatusCode(status, error) => EventError::new(
				ErrorCode::Network,
				format!("server returned status code {}: {}", status, String::from_utf8_lossy(&error)),
				fatal,
			),
		})
	}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, tsify::Tsify)]
#[tsify(namespace)]
pub enum ErrorCode {
	Network,
	Decode,
	Other,
}
