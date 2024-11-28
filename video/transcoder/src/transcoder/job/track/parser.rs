use std::io;

use anyhow::anyhow;
use bytes::{Buf, Bytes, BytesMut};
use bytesio::bytes_reader::BytesCursor;
use mp4::types::moov::Moov;
use mp4::types::trun::TrunSample;
use mp4::DynBox;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum TrackOut {
	// a Ftyp and Moov box are always sent at the start of a stream
	Moov(Moov),
	// A moof and mdat box are sent for each segment
	Samples(Vec<TrackSample>),
}

#[derive(Debug, Clone)]
pub struct TrackSample {
	pub duration: u32,
	pub keyframe: bool,
	pub sample: TrunSample,
	pub data: Bytes,
}

pub struct TrackParser {
	buffer: BytesMut,
	rx: mpsc::Receiver<Vec<u8>>,
	cursor: io::Cursor<Bytes>,
}

impl TrackParser {
	pub fn new(rx: mpsc::Receiver<Vec<u8>>) -> Self {
		Self {
			buffer: BytesMut::new(),
			rx,
			cursor: io::Cursor::new(Bytes::new()),
		}
	}

	pub async fn parse(&mut self) -> anyhow::Result<Option<TrackOut>> {
		loop {
			if let Some(out) = self.try_parse()? {
				return Ok(Some(out));
			}

			if let Some(data) = self.rx.recv().await {
				self.feed(&data);
			} else {
				return Ok(None);
			}
		}
	}

	fn feed(&mut self, data: &[u8]) {
		if self.cursor.has_remaining() {
			self.buffer.extend_from_slice(&self.cursor.extract_remaining());
		}

		self.buffer.extend_from_slice(data);
	}

	fn try_parse(&mut self) -> anyhow::Result<Option<TrackOut>> {
		if !self.cursor.has_remaining() {
			if self.buffer.is_empty() {
				return Ok(None);
			}

			self.cursor = io::Cursor::new(self.buffer.split().freeze());
		}

		while self.cursor.has_remaining() {
			let position = self.cursor.position() as usize;
			let b = match mp4::DynBox::demux(&mut self.cursor) {
				Ok(b) => b,
				Err(e) => {
					if e.kind() == io::ErrorKind::UnexpectedEof {
						// We need more data to parse this box
						self.cursor.set_position(position as u64);
						return Ok(None);
					}

					anyhow::bail!(e);
				}
			};

			match b {
				mp4::DynBox::Moov(moov) => {
					if moov.traks.len() != 1 {
						anyhow::bail!("moov box must have exactly one trak box");
					}

					return Ok(Some(TrackOut::Moov(moov)));
				}
				mp4::DynBox::Moof(moof) => {
					if moof.traf.len() != 1 {
						anyhow::bail!("moof box must have exactly one traf box");
					}

					let traf = &moof.traf[0];
					let trun = traf.trun.as_ref().ok_or_else(|| {
						io::Error::new(io::ErrorKind::InvalidData, anyhow!("traf box must have a trun box"))
					})?;
					let tfhd = &traf.tfhd;

					let samples = trun.samples.iter().enumerate().map(|(idx, sample)| {
						let mut sample = sample.clone();
						sample.duration = sample.duration.or(tfhd.default_sample_duration);
						sample.size = sample.size.or(tfhd.default_sample_size);
						sample.flags = Some(
							sample
								.flags
								.or(if idx == 0 { trun.first_sample_flags } else { None })
								.or(tfhd.default_sample_flags)
								.unwrap_or_default(),
						);
						sample.composition_time_offset = Some(sample.composition_time_offset.unwrap_or_default());
						sample
					});

					// Get the mdat box
					let mdat = match mp4::DynBox::demux(&mut self.cursor) {
						Ok(DynBox::Mdat(mdat)) => mdat,
						Ok(_) => {
							anyhow::bail!("mdat box must be the first box after moof");
						}
						Err(e) => {
							if e.kind() == io::ErrorKind::UnexpectedEof {
								// We need more data to parse this box
								self.cursor.set_position(position as u64);
								return Ok(None);
							}

							anyhow::bail!(e);
						}
					};

					if mdat.data.len() != 1 {
						anyhow::bail!("mdat box must have exactly one data box");
					}

					let mut mdat_cursor = io::Cursor::new(mdat.data[0].clone());
					return Ok(Some(TrackOut::Samples(
						samples
							.map(|sample| {
								let data = if let Some(size) = sample.size {
									mdat_cursor.read_slice(size as usize).map_err(|e| {
										io::Error::new(
											io::ErrorKind::InvalidData,
											anyhow!("mdat data size not big enough for sample: {}", e),
										)
									})?
								} else {
									mdat_cursor.extract_remaining()
								};

								io::Result::Ok(TrackSample {
									duration: sample.duration.unwrap_or_default(),
									keyframe: sample.flags.map(|f| f.sample_depends_on == 2).unwrap_or_default(),
									sample,
									data,
								})
							})
							.collect::<Result<Vec<_>, _>>()?,
					)));
				}
				_ => {}
			}
		}

		Ok(None)
	}
}
