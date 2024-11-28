use std::mem::MaybeUninit;
use std::sync::Arc;

use bytes::{Bytes, BytesMut};
use http_body::Body;
use io::{BodyEncoding, BodyParser};
use smallvec::SmallVec;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

use super::config::{Http1Builder, TcpServerConfigInner};
use crate::backend::tcp::TcpIncomingBody;
use crate::body::{has_body, IncomingBody};
use crate::error::{ErrorConfig, ErrorScope, ErrorSeverity, ResultErrorExt};
use crate::svc::ConnectionHandle;

pub mod io;
mod proto;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConnectionState {
	KeepAlive,
	Close,
	Upgrade,
	NotSpecified(http::Version),
}

pub async fn serve(
	stream: impl tokio::io::AsyncRead + tokio::io::AsyncWrite + Send + Sync + Unpin + 'static,
	addr: std::net::SocketAddr,
	handle: &Arc<impl ConnectionHandle>,
	config: TcpServerConfigInner,
) -> Result<(), crate::Error> {
	tracing::info!("serving http1");

	let http1_builder = config.http1_builder.ok_or_else(|| {
		crate::Error::new().with_config(ErrorConfig {
			severity: ErrorSeverity::Error,
			scope: ErrorScope::Connection,
			context: "http1 builder not configured",
		})
	})?;

	let (reader, writer) = tokio::io::split(stream);

	let mut reader = tokio::io::BufReader::with_capacity(http1_builder.recv_buffer_size, reader);
	let mut writer = tokio::io::BufWriter::with_capacity(http1_builder.send_buffer_size, writer);

	// Get a new request
	loop {
		let request = match config.idle_timeout {
			Some(idle_timeout) => match tokio::time::timeout(idle_timeout, parse_request(&http1_builder, &mut reader)).await
			{
				Ok(request) => request?,
				Err(_) => break,
			},
			None => parse_request(&http1_builder, &mut reader).await?,
		};

		let Some(mut request) = request else {
			break;
		};

		request.extensions_mut().insert(addr);

		let connection_state = handle_request(request, handle, &config, &http1_builder, &mut reader, &mut writer).await?;

		if connection_state != ConnectionState::KeepAlive {
			break;
		}
	}

	tracing::info!("connection closed");

	Ok(())
}

async fn handle_request(
	request: http::Request<()>,
	handle: &Arc<impl ConnectionHandle>,
	config: &TcpServerConfigInner,
	http1_builder: &Http1Builder,
	reader: &mut tokio::io::BufReader<impl tokio::io::AsyncRead + Unpin>,
	writer: &mut tokio::io::BufWriter<impl tokio::io::AsyncWrite + Unpin>,
) -> Result<ConnectionState, crate::Error> {
	let connection = request
		.headers()
		.get(http::header::CONNECTION)
		.map(|v| v.to_str())
		.transpose()
		.with_config(ErrorConfig {
			severity: ErrorSeverity::Debug,
			scope: ErrorScope::Request,
			context: "bad connection header",
		})?;

	let content_length = request
		.headers()
		.get(http::header::CONTENT_LENGTH)
		.map(|v| v.to_str())
		.transpose()
		.with_config(ErrorConfig {
			severity: ErrorSeverity::Debug,
			scope: ErrorScope::Request,
			context: "bad content length header",
		})?
		.map(|s| s.parse())
		.transpose()
		.with_config(ErrorConfig {
			severity: ErrorSeverity::Debug,
			scope: ErrorScope::Request,
			context: "bad content length header",
		})?;

	let mut is_chunked = false;

	if let Some(encodings) = request
		.headers()
		.get(http::header::TRANSFER_ENCODING)
		.map(|v| v.to_str())
		.transpose()
		.with_config(ErrorConfig {
			severity: ErrorSeverity::Debug,
			scope: ErrorScope::Request,
			context: "bad transfer encoding header",
		})?
		.map(|s| s.split(',').map(|s| s.trim()).collect::<SmallVec<[_; 16]>>())
	{
		for encoding in encodings {
			if encoding.eq_ignore_ascii_case("chunked") {
				is_chunked = true;
			} else {
				return Err(ErrorConfig {
					severity: ErrorSeverity::Debug,
					scope: ErrorScope::Request,
					context: "bad transfer encoding header",
				}
				.into());
			}
		}
	}

	let mut connection_state = match connection {
		Some(c) if c.eq_ignore_ascii_case("keep-alive") => ConnectionState::KeepAlive,
		Some(c) if c.eq_ignore_ascii_case("close") => ConnectionState::Close,
		Some(c) if c.eq_ignore_ascii_case("upgrade") => ConnectionState::Upgrade,
		Some(_) => {
			return Err(ErrorConfig {
				severity: ErrorSeverity::Debug,
				scope: ErrorScope::Request,
				context: "bad connection header",
			}
			.into())
		}
		None => ConnectionState::NotSpecified(request.version()),
	};

	let (body_parse, request) = if has_body(request.method()) {
		let encoding = match (content_length, is_chunked) {
			(Some(_), true) => {
				return Err(ErrorConfig {
					severity: ErrorSeverity::Debug,
					scope: ErrorScope::Request,
					context: "cannot have both content length and chunked transfer encoding",
				}
				.into());
			}
			(None, true) => BodyEncoding::Chunked {
				max_headers: http1_builder.max_headers,
			},
			(Some(content_length), false) => BodyEncoding::ContentLength(content_length),
			(None, false) => {
				return Err(ErrorConfig {
					severity: ErrorSeverity::Debug,
					scope: ErrorScope::Request,
					context: "no content length or transfer encoding specified",
				}
				.into())
			}
		};

		let (tx, rx) = tokio::sync::mpsc::channel::<http_body::Frame<Bytes>>(1);
		(
			BodyParser::reader(reader, tx, encoding),
			request.map(|()| IncomingBody::new(TcpIncomingBody::Http1(io::RecvStream::new(rx, content_length)))),
		)
	} else {
		(BodyParser::empty(), request.map(|_| IncomingBody::empty()))
	};

	let response_future = async move {
		let response = handle.on_request(request).await.map_err(|e| e.into())?;

		let (parts, body) = response.into_parts();

		let mut response = http::Response::from_parts(parts, ());

		let mut buf = BytesMut::new();

		if !response.headers().contains_key(http::header::DATE) {
			response.headers_mut().insert(
				http::header::DATE,
				httpdate::fmt_http_date(std::time::SystemTime::now()).parse().unwrap(),
			);
		}

		if !response.headers().contains_key(http::header::SERVER) {
			if let Some(server) = &config.server_name {
				response.headers_mut().insert(http::header::SERVER, server.parse().unwrap());
			}
		}

		const CLOSE_CONNECTION: http::HeaderValue = http::HeaderValue::from_static("close");
		const KEEP_ALIVE_CONNECTION: http::HeaderValue = http::HeaderValue::from_static("keep-alive");
		const UPGRADE_CONNECTION: http::HeaderValue = http::HeaderValue::from_static("upgrade");

		if let Some(connection) = response.headers_mut().remove(http::header::CONNECTION) {
			if let Ok(connection) = connection.to_str() {
				if connection.eq_ignore_ascii_case("close") {
					connection_state = ConnectionState::Close;
				} else if connection.eq_ignore_ascii_case("keep-alive") {
					connection_state = ConnectionState::KeepAlive;
				} else if connection.eq_ignore_ascii_case("upgrade") {
					connection_state = ConnectionState::Upgrade;
				}
			}
		}

		match connection_state {
			ConnectionState::KeepAlive => {
				response.headers_mut().insert(http::header::CONNECTION, KEEP_ALIVE_CONNECTION);
			}
			ConnectionState::Close => {
				response.headers_mut().insert(http::header::CONNECTION, CLOSE_CONNECTION);
			}
			ConnectionState::NotSpecified(http::Version::HTTP_10) => {
				response.headers_mut().insert(http::header::CONNECTION, CLOSE_CONNECTION);
			}
			ConnectionState::NotSpecified(http::Version::HTTP_11) => {
				response.headers_mut().insert(http::header::CONNECTION, KEEP_ALIVE_CONNECTION);
			}
			ConnectionState::Upgrade => {
				response.headers_mut().insert(http::header::CONNECTION, UPGRADE_CONNECTION);
			}
			_ => {
				response.headers_mut().insert(http::header::CONNECTION, CLOSE_CONNECTION);
				connection_state = ConnectionState::Close;
			}
		}

		proto::write_response(&mut buf, response, body.size_hint().exact());

		writer.write_all(buf.as_ref()).await?;

		writer.flush().await?;

		io::copy_body(body, writer).await?;

		writer.flush().await?;

		Ok(connection_state)
	};

	match futures::future::select(std::pin::pin!(response_future), std::pin::pin!(body_parse.read())).await {
		futures::future::Either::Left((response, body_result)) => {
			let state = response?;
			body_result.await?;
			Ok(state)
		}
		futures::future::Either::Right((body_result, response)) => {
			body_result?;
			response.await
		}
	}
}

async fn parse_request(
	http1_builder: &Http1Builder,
	reader: &mut tokio::io::BufReader<impl tokio::io::AsyncRead + Unpin>,
) -> Result<Option<http::Request<()>>, crate::Error> {
	loop {
		let mut headers: SmallVec<[MaybeUninit<httparse::Header<'_>>; 128]> =
			smallvec::smallvec![MaybeUninit::uninit(); http1_builder.max_headers];

		let Some((size, req)) = parse_headers(reader.buffer(), &mut headers)? else {
			let size = reader.buffer().len();
			if size >= 8000 {
				return Err(ErrorConfig {
					severity: ErrorSeverity::Debug,
					scope: ErrorScope::Request,
					context: "incoming request headers too long",
				}
				.into());
			}

			drop(headers);
			reader.fill_buf().await?;
			if size == reader.buffer().len() {
				return Ok(None);
			}

			continue;
		};

		let method = req
			.method
			.and_then(|m| http::Method::from_bytes(m.as_bytes()).ok())
			.ok_or(ErrorConfig {
				severity: ErrorSeverity::Debug,
				scope: ErrorScope::Request,
				context: "invalid incoming request method",
			})?;

		let mut builder = http::Request::builder()
			.version(match req.version {
				Some(0) => http::Version::HTTP_10,
				Some(1) => http::Version::HTTP_11,
				_ => {
					return Err(ErrorConfig {
						severity: ErrorSeverity::Debug,
						scope: ErrorScope::Request,
						context: "invalid incoming request HTTP version",
					}
					.into())
				}
			})
			.method(method)
			.uri(req.path.unwrap_or("/"));

		for header in req.headers.iter() {
			builder = builder.header(header.name, header.value);
		}

		let request = builder.body(())?;

		drop(headers);
		reader.consume(size);

		return Ok(Some(request));
	}
}

fn parse_headers<'h, 'b>(
	buf: &'b [u8],
	headers: &'h mut SmallVec<[MaybeUninit<httparse::Header<'b>>; 128]>,
) -> Result<Option<(usize, httparse::Request<'h, 'b>)>, crate::Error> {
	let mut req = httparse::Request::new(&mut []);
	let res = req.parse_with_uninit_headers(buf, headers).with_config(ErrorConfig {
		severity: ErrorSeverity::Debug,
		scope: ErrorScope::Request,
		context: "parse request headers",
	})?;
	match res {
		httparse::Status::Complete(size) => Ok(Some((size, req))),
		httparse::Status::Partial => Ok(None),
	}
}
