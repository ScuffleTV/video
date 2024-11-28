use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::{Buf, BufMut, Bytes, BytesMut};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader, BufWriter};
use tokio::sync::mpsc;

use super::{proto, ResultErrorExt};
use crate::error::{ErrorConfig, ErrorScope, ErrorSeverity};

#[derive(Clone, Copy)]
enum State {
	SizeKnown(u64),
	SizeUnknown,
	BodyComplete,
	TrailerComplete,
}

pin_project_lite::pin_project! {
	pub struct RecvStream {
		#[pin]
		state: State,
		#[pin]
		inner: mpsc::Receiver<http_body::Frame<Bytes>>,
	}
}

impl RecvStream {
	pub fn new(inner: mpsc::Receiver<http_body::Frame<Bytes>>, size: Option<u64>) -> Self {
		Self {
			state: size.map_or(State::SizeUnknown, State::SizeKnown),
			inner,
		}
	}
}

impl http_body::Body for RecvStream {
	type Data = Bytes;
	type Error = std::io::Error;

	fn poll_frame(
		self: Pin<&mut Self>,
		cx: &mut Context<'_>,
	) -> Poll<Option<Result<http_body::Frame<Self::Data>, Self::Error>>> {
		let mut this = self.project();

		let remaining = match *this.state {
			State::SizeUnknown => None,
			State::SizeKnown(remaining) => Some(remaining),
			State::BodyComplete => None,
			State::TrailerComplete => return Poll::Ready(None),
		};

		match this.inner.poll_recv(cx) {
			Poll::Pending => Poll::Pending,
			Poll::Ready(Some(frame)) => {
				match frame.data_ref() {
					Some(data) => {
						if let Some(remaining) = remaining {
							if remaining >= data.len() as u64 {
								*this.state = State::SizeKnown(remaining - data.len() as u64);
							} else {
								*this.state = State::BodyComplete;
							}
						}
					}
					None => {
						// If we get trailers, the body is done.
						*this.state = State::TrailerComplete;
						return Poll::Ready(Some(Ok(frame)));
					}
				}

				Poll::Ready(Some(Ok(frame)))
			}
			Poll::Ready(None) => {
				if matches!(*this.state, State::BodyComplete | State::TrailerComplete) {
					Poll::Ready(None)
				} else {
					Poll::Ready(Some(Err(std::io::Error::new(
						std::io::ErrorKind::UnexpectedEof,
						"Request too short",
					))))
				}
			}
		}
	}

	fn is_end_stream(&self) -> bool {
		matches!(self.state, State::BodyComplete | State::TrailerComplete)
	}

	fn size_hint(&self) -> http_body::SizeHint {
		match self.state {
			State::SizeKnown(remaining) => http_body::SizeHint::with_exact(remaining),
			State::SizeUnknown => http_body::SizeHint::new(),
			State::BodyComplete => http_body::SizeHint::with_exact(0),
			State::TrailerComplete => http_body::SizeHint::with_exact(0),
		}
	}
}

pub enum BodyEncoding {
	Chunked { max_headers: usize },
	ContentLength(u64),
}

pub enum BodyParser<'a, R: AsyncRead + Unpin> {
	Empty,
	Reader {
		reader: &'a mut BufReader<R>,
		body_sender: mpsc::Sender<http_body::Frame<Bytes>>,
		encoding: BodyEncoding,
	},
}

impl<'a, R: AsyncRead + Unpin> BodyParser<'a, R> {
	pub fn empty() -> Self {
		Self::Empty
	}

	pub fn reader(
		reader: &'a mut BufReader<R>,
		body_sender: mpsc::Sender<http_body::Frame<Bytes>>,
		encoding: BodyEncoding,
	) -> Self {
		Self::Reader {
			reader,
			body_sender,
			encoding,
		}
	}

	pub async fn read(self) -> Result<(), crate::Error> {
		match self {
			Self::Empty => Ok(()),
			Self::Reader {
				reader,
				body_sender,
				encoding,
			} => match encoding {
				BodyEncoding::Chunked { max_headers } => {
					// Loop for each chunk.
					loop {
						let chunk_size = httparse::parse_chunk_size(reader.buffer()).map_err(|_| ErrorConfig {
							severity: ErrorSeverity::Debug,
							scope: ErrorScope::Connection,
							context: "parse chunk size",
						})?;

						let mut chunk_size = match chunk_size {
							httparse::Status::Complete((idx, size)) => {
								reader.consume(idx);
								size
							}
							httparse::Status::Partial => {
								if reader.buffer().len() > 128 {
									return Err(ErrorConfig {
										severity: ErrorSeverity::Debug,
										scope: ErrorScope::Connection,
										context: "chunk header too large",
									}
									.into());
								}

								reader.fill_buf().await?;
								continue;
							}
						};

						// A zero-length chunk indicates the end of the body.
						if chunk_size == 0 {
							break;
						}

						while (reader.buffer().len() as u64) < chunk_size {
							if chunk_size != 0 && reader.buffer().is_empty() {
								reader.fill_buf().await?;
								if reader.buffer().is_empty() {
									return Err(ErrorConfig {
										severity: ErrorSeverity::Debug,
										scope: ErrorScope::Connection,
										context: "Request too short",
									}
									.into());
								}
							}

							let chunk = Bytes::copy_from_slice(reader.buffer());
							reader.consume(chunk.len());
							chunk_size -= chunk.len() as u64;
							body_sender.send(http_body::Frame::data(chunk)).await.ok();
						}

						if chunk_size != 0 {
							let chunk = Bytes::copy_from_slice(&reader.buffer()[..chunk_size as usize]);
							reader.consume(chunk.len());
							body_sender.send(http_body::Frame::data(chunk)).await.ok();
						}
					}

					loop {
						let Some((idx, headers)) = proto::parse_headers(reader.buffer(), max_headers)? else {
							if reader.buffer().len() > 1024 {
								return Err(ErrorConfig {
									severity: ErrorSeverity::Debug,
									scope: ErrorScope::Connection,
									context: "trailers too large",
								}
								.into());
							}

							let size = reader.buffer().len();
							reader.fill_buf().await?;
							if reader.buffer().len() == size {
								return Err(ErrorConfig {
									severity: ErrorSeverity::Debug,
									scope: ErrorScope::Connection,
									context: "Request too short",
								}
								.into());
							}

							continue;
						};

						reader.consume(idx);
						body_sender.send(http_body::Frame::trailers(headers)).await.ok();
						break;
					}

					Ok(())
				}
				BodyEncoding::ContentLength(mut remaining) => {
					while remaining > 0 {
						reader.fill_buf().await?;

						if reader.buffer().is_empty() {
							return Err(ErrorConfig {
								severity: ErrorSeverity::Debug,
								scope: ErrorScope::Connection,
								context: "Request too short",
							}
							.into());
						}

						let should_consume = reader.buffer().len().min(remaining as usize);
						let chunk = Bytes::copy_from_slice(&reader.buffer()[..should_consume]);

						remaining -= chunk.len() as u64;
						reader.consume(chunk.len());

						// I am not sure if we should ignore errors here.
						body_sender.send(http_body::Frame::data(chunk)).await.ok();
					}

					Ok(())
				}
			},
		}
	}
}

pub async fn copy_body<E: Into<crate::Error>>(
	body: impl http_body::Body<Error = E>,
	writer: &mut BufWriter<impl AsyncWrite + Unpin>,
) -> Result<(), crate::Error> {
	let mut size = body.size_hint().exact();

	let mut body = std::pin::pin!(body);

	let mut chunked_buf = BytesMut::new();

	while let Some(frame) = std::future::poll_fn(|cx| body.as_mut().poll_frame(cx)).await {
		chunked_buf.clear();

		match frame {
			Ok(mut frame) => {
				if let Some(data) = frame.data_mut() {
					match &mut size {
						Some(remaining) => {
							while *remaining > 0 && data.has_remaining() {
								let chunk = data.chunk();
								let size = chunk.len().min(*remaining as usize);
								writer.write_all(&chunk[..size]).await.with_context("body write")?;
								*remaining -= size as u64;
								data.advance(size);
							}
						}
						None => {
							let size = data.remaining();

							// TODO we should not use std::fmt here as this is very slow & does small heap
							// allocations.
							let hex_size = format!("{:x}", size as u64);

							chunked_buf.put_slice(hex_size.as_bytes());
							chunked_buf.put_slice(b"\r\n");

							writer
								.write_all(chunked_buf.as_ref())
								.await
								.with_context("body write chunk header")?;

							// We should take advantage of vectored writes here maybe?
							while data.has_remaining() {
								let chunk = data.chunk();
								writer.write_all(chunk).await.with_context("body write chunk")?;
								data.advance(chunk.len());
							}

							writer.write_all(b"\r\n").await.with_context("body write chunk trailer")?;
							writer.flush().await.with_context("body flush")?;
						}
					}
				} else if let Some(trailers) = frame.trailers_ref() {
					if size.is_some() {
						return Err(ErrorConfig {
							severity: ErrorSeverity::Debug,
							scope: ErrorScope::Connection,
							context: "Trailers specified with non-chunked encoding",
						}
						.into());
					}

					chunked_buf.put_slice(b"0\r\n");
					proto::write_headers(&mut chunked_buf, trailers);
					writer
						.write_all(chunked_buf.as_ref())
						.await
						.with_context("body write chunk trailers")?;
					return Ok(());
				}
			}
			Err(e) => {
				return Err(e.into().with_config(ErrorConfig {
					severity: ErrorSeverity::Error,
					scope: ErrorScope::Connection,
					context: "copy body",
				}));
			}
		}

		if size.is_some_and(|s| s == 0) {
			break;
		}
	}

	if size.is_none() {
		writer
			.write_all(b"0\r\n\r\n")
			.await
			.with_context("body write chunk trailers")?;
	}

	Ok(())
}
