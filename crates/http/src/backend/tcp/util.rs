#[cfg(feature = "tls-rustls")]
use std::sync::Arc;

#[cfg(feature = "tls-rustls")]
use bytes::{BufMut, Bytes, BytesMut};
#[cfg(feature = "tls-rustls")]
use tokio::io::AsyncWriteExt;

#[cfg(feature = "tls-rustls")]
use crate::svc::ConnectionHandle;

pub fn is_fatal_tcp_error(err: &std::io::Error) -> bool {
	matches!(
		err.raw_os_error(),
		Some(libc::EFAULT)
			| Some(libc::EINVAL)
			| Some(libc::ENFILE)
			| Some(libc::EMFILE)
			| Some(libc::ENOBUFS)
			| Some(libc::ENOMEM)
	)
}

#[cfg(feature = "tls-rustls")]
pub async fn is_tls(stream: &mut tokio::net::TcpStream, handle: &Arc<impl ConnectionHandle>) -> bool {
	const H2_PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

	let mut buf = [0; 24];
	let n = match stream.peek(&mut buf).await {
		Ok(n) => n,
		Err(e) => {
			handle.on_error(e.into());
			return false;
		}
	};

	if &buf[..n] == H2_PREFACE {
		return false;
	}

	if std::str::from_utf8(&buf[..n]).is_ok_and(|buf| buf.contains("HTTP/1.1")) {
		stream.write_all(&make_bad_response(BAD_REQUEST_PLAIN_ON_TLS)).await.ok();
		return false;
	}

	if n < 3 || buf[0] != 0x16 || buf[1] != 0x03 || buf[2] < 0x01 {
		stream.write_all(&make_bad_response(BAD_REQUEST_BODY)).await.ok();
		return false;
	}

	true
}

#[cfg(feature = "tls-rustls")]
const BAD_REQUEST_BODY: &str = "\
<html>
<head><title>400 Bad Request</title></head>
<body>
<center><h1>400 Bad Request</h1></center>
</body>
</html>";

#[cfg(feature = "tls-rustls")]
const BAD_REQUEST_PLAIN_ON_TLS: &str = "\
<html>
<head><title>400 Sent plain HTTP request to an HTTPS port</title></head>
<body>
<center><h1>400</h1></center>
<center>Sent plain HTTP request to an HTTPS port</center>
</body>
</html>";

#[cfg(feature = "tls-rustls")]
pub fn make_bad_response(message: &'static str) -> Bytes {
	let mut buf = BytesMut::new();
	buf.put_slice(b"HTTP/1.1 400 Bad Request\r\n");
	buf.put_slice(b"Content-Type: text/html\r\n");
	buf.put_slice(b"Date: ");
	buf.put_slice(httpdate::fmt_http_date(std::time::SystemTime::now()).as_bytes());
	buf.put_slice(b"\r\n");
	buf.put_slice(b"Connection: close\r\n");
	buf.put_slice(b"Content-Length: ");
	buf.put_slice(itoa::Buffer::new().format(message.len()).as_bytes());
	buf.put_slice(b"\r\n\r\n");
	buf.put_slice(message.as_bytes());
	buf.freeze()
}
