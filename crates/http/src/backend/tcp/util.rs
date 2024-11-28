use std::pin::Pin;
#[cfg(feature = "tls-rustls")]
use std::sync::Arc;
use std::task::Poll;

#[cfg(feature = "tls-rustls")]
use bytes::{BufMut, Bytes, BytesMut};
#[cfg(feature = "tls-rustls")]
use tokio::io::AsyncWriteExt;

#[cfg(feature = "tls-rustls")]
use crate::svc::ConnectionHandle;

const H2_PREFACE: &[u8] = b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n";

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

#[cfg(all(feature = "http1", feature = "http2"))]
pub enum Version {
	H2,
	H1,
}

#[cfg(all(feature = "http1", feature = "http2"))]
pub struct Rewind<I> {
	inner: I,
	buf: [u8; 24],
	pos: u8,
}

#[cfg(all(feature = "http1", feature = "http2"))]
impl<I: tokio::io::AsyncRead + Unpin> tokio::io::AsyncRead for Rewind<I> {
	fn poll_read(
		mut self: Pin<&mut Self>,
		cx: &mut std::task::Context<'_>,
		buf: &mut tokio::io::ReadBuf<'_>,
	) -> Poll<std::io::Result<()>> {
		if self.pos != 0 {
			let n = (self.pos as usize).min(buf.remaining());
			let start = 24 - self.pos as usize;
			let end = start + n;
			buf.put_slice(&self.buf[start..end]);
			self.pos -= n as u8;

			// If we have no remaining bytes to fill, we should return ready and wake the
			// task This is because we dont drive the underlying reader.
			if buf.remaining() == 0 {
				cx.waker().wake_by_ref();
				return Poll::Ready(Ok(()));
			}
		}

		Pin::new(&mut self.inner).poll_read(cx, buf)
	}
}

#[cfg(all(feature = "http1", feature = "http2"))]
impl<I: tokio::io::AsyncWrite + Unpin> tokio::io::AsyncWrite for Rewind<I> {
	fn poll_write(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>, buf: &[u8]) -> Poll<std::io::Result<usize>> {
		Pin::new(&mut self.inner).poll_write(cx, buf)
	}

	fn poll_flush(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<std::io::Result<()>> {
		Pin::new(&mut self.inner).poll_flush(cx)
	}

	fn is_write_vectored(&self) -> bool {
		self.inner.is_write_vectored()
	}

	fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<std::io::Result<()>> {
		Pin::new(&mut self.inner).poll_shutdown(cx)
	}

	fn poll_write_vectored(
		mut self: Pin<&mut Self>,
		cx: &mut std::task::Context<'_>,
		bufs: &[std::io::IoSlice<'_>],
	) -> Poll<Result<usize, std::io::Error>> {
		Pin::new(&mut self.inner).poll_write_vectored(cx, bufs)
	}
}

#[cfg(all(feature = "http1", feature = "http2"))]
impl<I> Rewind<I> {
	fn new(inner: I, buf: [u8; 24]) -> Self {
		Self { inner, buf, pos: 24 }
	}
}

#[cfg(all(feature = "http1", feature = "http2"))]
pub async fn read_version<I: tokio::io::AsyncRead + Unpin>(mut io: I) -> std::io::Result<(Version, Rewind<I>)> {
	use tokio::io::AsyncReadExt;
	let mut buf = [0; 24];
	io.read_exact(&mut buf).await?;
	let rewind = Rewind::new(io, buf);

	if buf == H2_PREFACE {
		Ok((Version::H2, rewind))
	} else {
		Ok((Version::H1, rewind))
	}
}
