use std::io;

use bytes::{Buf, Bytes};

use super::elst::Elst;
use crate::boxes::header::BoxHeader;
use crate::boxes::traits::BoxType;
use crate::boxes::DynBox;

#[derive(Debug, Clone, PartialEq)]
/// Edit Box
/// ISO/IEC 14496-12:2022(E) 8.6.5
pub struct Edts {
	pub header: BoxHeader,
	pub elst: Option<Elst>,
	pub unknown: Vec<DynBox>,
}

impl Edts {
	pub fn new(elst: Option<Elst>) -> Self {
		Self {
			header: BoxHeader::new(Self::NAME),
			elst,
			unknown: Vec::new(),
		}
	}
}

impl BoxType for Edts {
	const NAME: [u8; 4] = *b"edts";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);
		let mut elst = None;
		let mut unknown = Vec::new();

		while reader.has_remaining() {
			let dyn_box = DynBox::demux(&mut reader)?;

			match dyn_box {
				DynBox::Elst(b) => {
					elst = Some(b);
				}
				_ => {
					unknown.push(dyn_box);
				}
			}
		}

		Ok(Self { header, elst, unknown })
	}

	fn primitive_size(&self) -> u64 {
		self.elst.iter().map(|b| b.size()).sum::<u64>() + self.unknown.iter().map(|b| b.size()).sum::<u64>()
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		for b in &self.elst {
			b.mux(writer)?;
		}

		for b in &self.unknown {
			b.mux(writer)?;
		}

		Ok(())
	}
}
