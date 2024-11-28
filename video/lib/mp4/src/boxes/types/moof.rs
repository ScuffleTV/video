use std::io;

use bytes::{Buf, Bytes};

use super::mfhd::Mfhd;
use super::traf::Traf;
use crate::boxes::header::BoxHeader;
use crate::boxes::traits::BoxType;
use crate::boxes::DynBox;

#[derive(Debug, Clone, PartialEq)]
/// Movie Fragment Box
/// ISO/IEC 14496-12:2022(E) - 8.8.4
pub struct Moof {
	pub header: BoxHeader,
	pub mfhd: Mfhd,
	pub traf: Vec<Traf>,
	pub unknown: Vec<DynBox>,
}

impl Moof {
	pub fn new(mfhd: Mfhd, traf: Vec<Traf>) -> Self {
		Self {
			header: BoxHeader::new(Self::NAME),
			mfhd,
			traf,
			unknown: Vec::new(),
		}
	}
}

impl BoxType for Moof {
	const NAME: [u8; 4] = *b"moof";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let mut unknown = Vec::new();

		let mut traf = Vec::new();
		let mut mfhd = None;

		while reader.has_remaining() {
			let box_ = DynBox::demux(&mut reader)?;
			match box_ {
				DynBox::Mfhd(b) => {
					mfhd = Some(b);
				}
				DynBox::Traf(b) => {
					traf.push(b);
				}
				_ => unknown.push(box_),
			}
		}

		let mfhd = mfhd.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "moof box must contain mfhd box"))?;

		Ok(Self {
			header,
			mfhd,
			traf,
			unknown,
		})
	}

	fn primitive_size(&self) -> u64 {
		self.mfhd.size()
			+ self.traf.iter().map(|box_| box_.size()).sum::<u64>()
			+ self.unknown.iter().map(|box_| box_.size()).sum::<u64>()
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.mfhd.mux(writer)?;

		for box_ in &self.traf {
			box_.mux(writer)?;
		}

		for box_ in &self.unknown {
			box_.mux(writer)?;
		}

		Ok(())
	}
}
