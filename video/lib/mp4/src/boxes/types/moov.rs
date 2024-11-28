use std::io;

use bytes::{Buf, Bytes};

use super::mvex::Mvex;
use super::mvhd::Mvhd;
use super::trak::Trak;
use crate::boxes::header::BoxHeader;
use crate::boxes::traits::BoxType;
use crate::boxes::DynBox;

#[derive(Debug, Clone, PartialEq)]
/// Movie Box
/// ISO/IEC 14496-12:2022(E) - 8.2.1
pub struct Moov {
	pub header: BoxHeader,
	pub mvhd: Mvhd,
	pub traks: Vec<Trak>,
	pub mvex: Option<Mvex>,
	pub unknown: Vec<DynBox>,
}

impl Moov {
	pub fn new(mvhd: Mvhd, traks: Vec<Trak>, mvex: Option<Mvex>) -> Self {
		Self {
			header: BoxHeader::new(Self::NAME),
			mvhd,
			traks,
			mvex,
			unknown: Vec::new(),
		}
	}
}

impl BoxType for Moov {
	const NAME: [u8; 4] = *b"moov";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let mut traks = Vec::new();
		let mut mvex = None;
		let mut mvhd = None;
		let mut unknown = Vec::new();

		while reader.has_remaining() {
			let dyn_box = DynBox::demux(&mut reader)?;

			match dyn_box {
				DynBox::Mvhd(b) => {
					mvhd = Some(b);
				}
				DynBox::Trak(b) => {
					traks.push(b);
				}
				DynBox::Mvex(b) => {
					mvex = Some(b);
				}
				_ => {
					unknown.push(dyn_box);
				}
			}
		}

		let mvhd = mvhd.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "moov box is missing mvhd box"))?;

		Ok(Self {
			header,
			mvhd,
			traks,
			mvex,
			unknown,
		})
	}

	fn primitive_size(&self) -> u64 {
		self.mvhd.size()
			+ self.traks.iter().map(|b| b.size()).sum::<u64>()
			+ self.mvex.as_ref().map(|b| b.size()).unwrap_or(0)
			+ self.unknown.iter().map(|b| b.size()).sum::<u64>()
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.mvhd.mux(writer)?;

		for trak in &self.traks {
			trak.mux(writer)?;
		}

		if let Some(mvex) = &self.mvex {
			mvex.mux(writer)?;
		}

		for unknown in &self.unknown {
			unknown.mux(writer)?;
		}

		Ok(())
	}
}
