use std::io;

use bytes::{Buf, Bytes};

use super::dinf::Dinf;
use super::hmhd::Hmhd;
use super::nmhd::Nmhd;
use super::smhd::Smhd;
use super::stbl::Stbl;
use super::vmhd::Vmhd;
use crate::boxes::header::BoxHeader;
use crate::boxes::traits::BoxType;
use crate::boxes::DynBox;

#[derive(Debug, Clone, PartialEq)]
/// Media Information Box
/// ISO/IEC 14496-12:2022(E) - 8.4.4
pub struct Minf {
	pub header: BoxHeader,
	pub vmhd: Option<Vmhd>,
	pub smhd: Option<Smhd>,
	pub hmhd: Option<Hmhd>,
	pub nmhd: Option<Nmhd>,
	pub dinf: Dinf,
	pub stbl: Stbl,
	pub unknown: Vec<DynBox>,
}

impl Minf {
	pub fn new(stbl: Stbl, vmhd: Option<Vmhd>, smhd: Option<Smhd>) -> Self {
		Self {
			header: BoxHeader::new(Self::NAME),
			vmhd,
			smhd,
			hmhd: None,
			nmhd: None,
			dinf: Dinf::new(),
			stbl,
			unknown: Vec::new(),
		}
	}
}

impl BoxType for Minf {
	const NAME: [u8; 4] = *b"minf";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);
		let mut vmhd = None;
		let mut smhd = None;
		let mut hmhd = None;
		let mut nmhd = None;
		let mut dinf = None;
		let mut stbl = None;
		let mut unknown = Vec::new();

		while reader.has_remaining() {
			let dyn_box = DynBox::demux(&mut reader)?;

			match dyn_box {
				DynBox::Vmhd(b) => {
					vmhd = Some(b);
				}
				DynBox::Smhd(b) => {
					smhd = Some(b);
				}
				DynBox::Hmhd(b) => {
					hmhd = Some(b);
				}
				DynBox::Nmhd(b) => {
					nmhd = Some(b);
				}
				DynBox::Dinf(b) => {
					dinf = Some(b);
				}
				DynBox::Stbl(b) => {
					stbl = Some(b);
				}
				_ => {
					unknown.push(dyn_box);
				}
			}
		}

		let dinf = dinf.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "minf: dinf box is required"))?;
		let stbl = stbl.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "minf: stbl box is required"))?;

		Ok(Self {
			header,
			vmhd,
			smhd,
			hmhd,
			nmhd,
			dinf,
			stbl,
			unknown,
		})
	}

	fn primitive_size(&self) -> u64 {
		self.vmhd.as_ref().map(|b| b.size()).unwrap_or(0) // vmhd 
        + self.smhd.as_ref().map(|b| b.size()).unwrap_or(0) // smhd
        + self.hmhd.as_ref().map(|b| b.size()).unwrap_or(0) // hmhd
        + self.nmhd.as_ref().map(|b| b.size()).unwrap_or(0) // nmhd
        + self.dinf.size() // dinf
        + self.stbl.size() // stbl
        + self.unknown.iter().map(|b| b.size()).sum::<u64>() // unknown boxes
	}

	fn primitive_mux<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
		if let Some(b) = &self.vmhd {
			b.mux(writer)?;
		}

		if let Some(b) = &self.smhd {
			b.mux(writer)?;
		}

		if let Some(b) = &self.hmhd {
			b.mux(writer)?;
		}

		if let Some(b) = &self.nmhd {
			b.mux(writer)?;
		}

		self.dinf.mux(writer)?;

		self.stbl.mux(writer)?;

		for b in &self.unknown {
			b.mux(writer)?;
		}

		Ok(())
	}
}
