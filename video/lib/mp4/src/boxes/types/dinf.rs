use std::io;

use bytes::{Buf, Bytes};

use crate::boxes::header::BoxHeader;
use crate::boxes::traits::BoxType;
use crate::boxes::types::dref::Dref;
use crate::boxes::DynBox;

#[derive(Debug, Clone, PartialEq)]
/// Data Information Box
/// ISO/IEC 14496-12:2022(E) 8.7.1
pub struct Dinf {
	pub header: BoxHeader,
	pub dref: Dref,
	pub unknown: Vec<DynBox>,
}

impl Default for Dinf {
	fn default() -> Self {
		Self::new()
	}
}

impl Dinf {
	pub fn new() -> Self {
		Self {
			header: BoxHeader::new(Self::NAME),
			dref: Dref::new(),
			unknown: Vec::new(),
		}
	}
}

impl BoxType for Dinf {
	const NAME: [u8; 4] = *b"dinf";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let mut dref = None;
		let mut unknown = Vec::new();

		while reader.has_remaining() {
			let dyn_box = DynBox::demux(&mut reader)?;

			match dyn_box {
				DynBox::Dref(b) => {
					dref = Some(b);
				}
				_ => {
					unknown.push(dyn_box);
				}
			}
		}

		Ok(Self {
			header,
			dref: dref.unwrap(),
			unknown,
		})
	}

	fn primitive_size(&self) -> u64 {
		self.dref.size() + self.unknown.iter().map(|b| b.size()).sum::<u64>()
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.dref.mux(writer)?;

		for b in &self.unknown {
			b.mux(writer)?;
		}

		Ok(())
	}
}
