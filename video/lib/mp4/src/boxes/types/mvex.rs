use std::io;

use bytes::{Buf, Bytes};

use super::mehd::Mehd;
use super::trex::Trex;
use crate::boxes::header::BoxHeader;
use crate::boxes::traits::BoxType;
use crate::boxes::DynBox;

#[derive(Debug, Clone, PartialEq)]
/// Movie Extends Box
/// ISO/IEC 14496-12:2022(E) - 8.8.1
pub struct Mvex {
	pub header: BoxHeader,
	pub trex: Vec<Trex>,
	pub mehd: Option<Mehd>,
	pub unknown: Vec<DynBox>,
}

impl Mvex {
	pub fn new(trex: Vec<Trex>, mehd: Option<Mehd>) -> Self {
		Self {
			header: BoxHeader::new(Self::NAME),
			trex,
			mehd,
			unknown: Vec::new(),
		}
	}
}

impl BoxType for Mvex {
	const NAME: [u8; 4] = *b"mvex";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut trex = Vec::new();
		let mut mehd = None;
		let mut unknown = Vec::new();

		let mut data = io::Cursor::new(data);
		while data.has_remaining() {
			let dyn_box = DynBox::demux(&mut data)?;

			match dyn_box {
				DynBox::Trex(b) => {
					trex.push(b);
				}
				DynBox::Mehd(b) => {
					mehd = Some(b);
				}
				_ => {
					unknown.push(dyn_box);
				}
			}
		}

		Ok(Self {
			header,
			trex,
			mehd,
			unknown,
		})
	}

	fn primitive_size(&self) -> u64 {
		self.trex.iter().map(|b| b.size()).sum::<u64>()
			+ self.mehd.iter().map(|b| b.size()).sum::<u64>()
			+ self.unknown.iter().map(|b| b.size()).sum::<u64>()
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		for b in &self.trex {
			b.mux(writer)?;
		}

		if let Some(b) = &self.mehd {
			b.mux(writer)?;
		}

		for b in &self.unknown {
			b.mux(writer)?;
		}

		Ok(())
	}
}
