use std::io;

use bytes::{Buf, Bytes};

use super::hdlr::Hdlr;
use super::mdhd::Mdhd;
use super::minf::Minf;
use crate::boxes::header::BoxHeader;
use crate::boxes::traits::BoxType;
use crate::boxes::DynBox;

#[derive(Debug, Clone, PartialEq)]
/// Media Box
/// ISO/IEC 14496-12:2022(E) - 8.4
pub struct Mdia {
	pub header: BoxHeader,
	pub mdhd: Mdhd,
	pub hdlr: Hdlr,
	pub minf: Minf,
	pub unknown: Vec<DynBox>,
}

impl Mdia {
	pub fn new(mdhd: Mdhd, hdlr: Hdlr, minf: Minf) -> Self {
		Self {
			header: BoxHeader::new(Self::NAME),
			mdhd,
			hdlr,
			minf,
			unknown: Vec::new(),
		}
	}
}

impl BoxType for Mdia {
	const NAME: [u8; 4] = *b"mdia";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);
		let mut mdhd = None;
		let mut hdlr = None;
		let mut minf = None;
		let mut unknown = Vec::new();

		while reader.has_remaining() {
			let dyn_box = DynBox::demux(&mut reader)?;

			match dyn_box {
				DynBox::Mdhd(b) => {
					mdhd = Some(b);
				}
				DynBox::Hdlr(b) => {
					hdlr = Some(b);
				}
				DynBox::Minf(b) => {
					minf = Some(b);
				}
				_ => {
					unknown.push(dyn_box);
				}
			}
		}

		let mdhd = mdhd.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "mdia box is missing mdhd box"))?;

		let hdlr = hdlr.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "mdia box is missing hdlr box"))?;

		let minf = minf.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "mdia box is missing minf box"))?;

		Ok(Self {
			header,
			mdhd,
			hdlr,
			minf,
			unknown,
		})
	}

	fn primitive_size(&self) -> u64 {
		self.mdhd.size() // mdhd
        + self.hdlr.size() // hdlr
        + self.minf.size() // minf
        + self.unknown.iter().map(|b| b.size()).sum::<u64>() // unknown boxes
	}

	fn primitive_mux<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
		self.mdhd.mux(writer)?;
		self.hdlr.mux(writer)?;
		self.minf.mux(writer)?;

		for b in &self.unknown {
			b.mux(writer)?;
		}

		Ok(())
	}
}
