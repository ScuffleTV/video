use std::io;

use bytes::{Buf, Bytes};

use super::edts::Edts;
use super::mdia::Mdia;
use super::tkhd::Tkhd;
use crate::boxes::header::BoxHeader;
use crate::boxes::traits::BoxType;
use crate::boxes::DynBox;

#[derive(Debug, Clone, PartialEq)]
/// Track Box
/// ISO/IEC 14496-12:2022(E) - 8.3.1
pub struct Trak {
	pub header: BoxHeader,
	pub tkhd: Tkhd,
	pub edts: Option<Edts>,
	pub mdia: Mdia,
	pub unknown: Vec<DynBox>,
}

impl Trak {
	pub fn new(tkhd: Tkhd, edts: Option<Edts>, mdia: Mdia) -> Self {
		Self {
			header: BoxHeader::new(Self::NAME),
			tkhd,
			edts,
			mdia,
			unknown: Vec::new(),
		}
	}
}

impl BoxType for Trak {
	const NAME: [u8; 4] = *b"trak";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);
		let mut tkhd = None;
		let mut edts = None;
		let mut mdia = None;
		let mut unknown = Vec::new();

		while reader.has_remaining() {
			let dyn_box = DynBox::demux(&mut reader)?;

			match dyn_box {
				DynBox::Tkhd(b) => {
					tkhd = Some(b);
				}
				DynBox::Edts(b) => {
					edts = Some(b);
				}
				DynBox::Mdia(b) => {
					mdia = Some(b);
				}
				_ => {
					unknown.push(dyn_box);
				}
			}
		}

		let tkhd = tkhd.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "trak box is missing tkhd box"))?;

		let mdia = mdia.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "trak box is missing mdia box"))?;

		Ok(Self {
			header,
			tkhd,
			edts,
			mdia,
			unknown,
		})
	}

	fn primitive_size(&self) -> u64 {
		self.tkhd.size()
			+ self.edts.as_ref().map(|b| b.size()).unwrap_or(0)
			+ self.mdia.size()
			+ self.unknown.iter().map(|b| b.size()).sum::<u64>()
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.tkhd.mux(writer)?;
		if let Some(edts) = &self.edts {
			edts.mux(writer)?;
		}

		self.mdia.mux(writer)?;

		for box_ in &self.unknown {
			box_.mux(writer)?;
		}

		Ok(())
	}
}
