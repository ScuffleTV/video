use bytes::{BufMut, BytesMut};
use http::{HeaderName, HeaderValue};
use smallvec::SmallVec;

use super::ResultErrorExt;
use crate::error::{ErrorConfig, ErrorScope, ErrorSeverity};

pub fn write_response(buf: &mut BytesMut, mut response: http::Response<()>, size: Option<u64>) {
	match response.version() {
		http::Version::HTTP_10 => buf.put_slice(b"HTTP/1.0 "),
		_ => buf.put_slice(b"HTTP/1.1 "),
	};

	buf.put_slice(itoa::Buffer::new().format(response.status().as_u16()).as_bytes());

	if let Some(reason) = response.status().canonical_reason() {
		buf.put_slice(b" ");
		buf.put_slice(reason.as_bytes());
	}

	buf.put_slice(b"\r\n");

	response.headers_mut().remove(http::header::CONTENT_LENGTH);
	response.headers_mut().remove(http::header::TRANSFER_ENCODING);

	match size {
		Some(size) => {
			response.headers_mut().insert(
				http::header::CONTENT_LENGTH,
				itoa::Buffer::new().format(size).parse().unwrap(),
			);
		}
		None => {
			response
				.headers_mut()
				.insert(http::header::TRANSFER_ENCODING, "chunked".parse().unwrap());
		}
	}

	write_headers(buf, response.headers());
}

pub fn write_headers(buf: &mut BytesMut, headers: &http::HeaderMap) {
	for (name, value) in headers.iter() {
		buf.put_slice(name.as_str().as_bytes());
		buf.put_slice(b": ");
		buf.put_slice(value.to_str().unwrap().as_bytes());
		buf.put_slice(b"\r\n");
	}

	buf.put_slice(b"\r\n");
}

pub fn parse_headers(buffer: &[u8], max_headers: usize) -> Result<Option<(usize, http::HeaderMap)>, crate::Error> {
	let mut headers: SmallVec<[httparse::Header<'_>; 128]> = smallvec::smallvec![httparse::EMPTY_HEADER; max_headers];
	let result = httparse::parse_headers(buffer, &mut headers).with_config(ErrorConfig {
		severity: ErrorSeverity::Debug,
		scope: ErrorScope::Request,
		context: "parse request headers",
	})?;
	match result {
		httparse::Status::Complete((idx, ref_headers)) => {
			let mut headers = http::HeaderMap::new();
			for header in ref_headers.iter() {
				if let Ok(value) = HeaderValue::from_bytes(header.value) {
					let Ok(name) = header.name.parse::<HeaderName>() else {
						continue;
					};

					headers.insert(name, value);
				}
			}

			Ok(Some((idx, headers)))
		}
		httparse::Status::Partial => Ok(None),
	}
}
