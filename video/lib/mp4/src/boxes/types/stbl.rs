use std::io;

use bytes::{Buf, Bytes};

use super::co64::Co64;
use super::ctts::Ctts;
use super::padb::Padb;
use super::sbgp::Sbgp;
use super::sdtp::Sdtp;
use super::stco::Stco;
use super::stdp::Stdp;
use super::stsc::Stsc;
use super::stsd::Stsd;
use super::stsh::Stsh;
use super::stss::Stss;
use super::stsz::Stsz;
use super::stts::Stts;
use super::stz2::Stz2;
use super::subs::Subs;
use crate::boxes::header::BoxHeader;
use crate::boxes::traits::BoxType;
use crate::boxes::DynBox;

#[derive(Debug, Clone, PartialEq)]
/// Sample Table Box
/// ISO/IEC 14496-12:2022(E) 8.5.1
pub struct Stbl {
	pub header: BoxHeader,
	pub stsd: Stsd,
	pub stts: Stts,
	pub ctts: Option<Ctts>,
	pub stsc: Stsc,
	pub stsz: Option<Stsz>,
	pub stz2: Option<Stz2>,
	pub stco: Stco,
	pub co64: Option<Co64>,
	pub stss: Option<Stss>,
	pub stsh: Option<Stsh>,
	pub padb: Option<Padb>,
	pub stdp: Option<Stdp>,
	pub sdtp: Option<Sdtp>,
	pub sbgp: Option<Sbgp>,
	pub subs: Option<Subs>,
	pub unknown: Vec<DynBox>,
}

impl Stbl {
	pub fn new(stsd: Stsd, stts: Stts, stsc: Stsc, stco: Stco, stsz: Option<Stsz>) -> Self {
		Self {
			header: BoxHeader::new(Self::NAME),
			stsd,
			stts,
			ctts: None,
			stsc,
			stsz,
			stz2: None,
			stco,
			co64: None,
			stss: None,
			stsh: None,
			padb: None,
			stdp: None,
			sdtp: None,
			sbgp: None,
			subs: None,
			unknown: Vec::new(),
		}
	}
}

impl BoxType for Stbl {
	const NAME: [u8; 4] = *b"stbl";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);
		let mut stsd = None;
		let mut stts = None;
		let mut ctts = None;
		let mut stsc = None;
		let mut stsz = None;
		let mut stz2 = None;
		let mut stco = None;
		let mut co64 = None;
		let mut stss = None;
		let mut stsh = None;
		let mut padb = None;
		let mut stdp = None;
		let mut sdtp = None;
		let mut sbgp = None;
		let mut subs = None;
		let mut unknown = Vec::new();

		while reader.has_remaining() {
			let dyn_box = DynBox::demux(&mut reader)?;

			match dyn_box {
				DynBox::Stsd(b) => {
					stsd = Some(b);
				}
				DynBox::Stts(b) => {
					stts = Some(b);
				}
				DynBox::Ctts(b) => {
					ctts = Some(b);
				}
				DynBox::Stsc(b) => {
					stsc = Some(b);
				}
				DynBox::Stsz(b) => {
					stsz = Some(b);
				}
				DynBox::Stz2(b) => {
					stz2 = Some(b);
				}
				DynBox::Stco(b) => {
					stco = Some(b);
				}
				DynBox::Co64(b) => {
					co64 = Some(b);
				}
				DynBox::Stss(b) => {
					stss = Some(b);
				}
				DynBox::Stsh(b) => {
					stsh = Some(b);
				}
				DynBox::Padb(b) => {
					padb = Some(b);
				}
				DynBox::Stdp(b) => {
					stdp = Some(b);
				}
				DynBox::Sdtp(b) => {
					sdtp = Some(b);
				}
				DynBox::Sbgp(b) => {
					sbgp = Some(b);
				}
				DynBox::Subs(b) => {
					subs = Some(b);
				}
				_ => {
					unknown.push(dyn_box);
				}
			}
		}

		let stsd = stsd.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "stsd box not found in stbl box"))?;
		let stts = stts.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "stts box not found in stbl box"))?;
		let stsc = stsc.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "stsc box not found in stbl box"))?;
		let stco = stco.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "stco box not found in stbl box"))?;

		Ok(Self {
			header,
			stsd,
			stts,
			ctts,
			stsc,
			stsz,
			stz2,
			stco,
			co64,
			stss,
			stsh,
			padb,
			stdp,
			sdtp,
			sbgp,
			subs,
			unknown,
		})
	}

	fn primitive_size(&self) -> u64 {
		let mut size = self.stsd.size();
		size += self.stts.size();
		size += self.ctts.as_ref().map(|b| b.size()).unwrap_or(0);
		size += self.stsc.size();
		size += self.stsz.as_ref().map(|b| b.size()).unwrap_or(0);
		size += self.stz2.as_ref().map(|b| b.size()).unwrap_or(0);
		size += self.stco.size();
		size += self.co64.as_ref().map(|b| b.size()).unwrap_or(0);
		size += self.stss.as_ref().map(|b| b.size()).unwrap_or(0);
		size += self.stsh.as_ref().map(|b| b.size()).unwrap_or(0);
		size += self.padb.as_ref().map(|b| b.size()).unwrap_or(0);
		size += self.stdp.as_ref().map(|b| b.size()).unwrap_or(0);
		size += self.sdtp.as_ref().map(|b| b.size()).unwrap_or(0);
		size += self.sbgp.as_ref().map(|b| b.size()).unwrap_or(0);
		size += self.subs.as_ref().map(|b| b.size()).unwrap_or(0);
		size += self.unknown.iter().map(|b| b.size()).sum::<u64>();
		size
	}

	fn primitive_mux<W: io::Write>(&self, writer: &mut W) -> io::Result<()> {
		self.stsd.mux(writer)?;
		self.stts.mux(writer)?;
		if let Some(ctts) = &self.ctts {
			ctts.mux(writer)?;
		}
		self.stsc.mux(writer)?;
		if let Some(stsz) = &self.stsz {
			stsz.mux(writer)?;
		}
		if let Some(stz2) = &self.stz2 {
			stz2.mux(writer)?;
		}
		self.stco.mux(writer)?;
		if let Some(co64) = &self.co64 {
			co64.mux(writer)?;
		}
		if let Some(stss) = &self.stss {
			stss.mux(writer)?;
		}
		if let Some(stsh) = &self.stsh {
			stsh.mux(writer)?;
		}
		if let Some(padb) = &self.padb {
			padb.mux(writer)?;
		}
		if let Some(stdp) = &self.stdp {
			stdp.mux(writer)?;
		}
		if let Some(sdtp) = &self.sdtp {
			sdtp.mux(writer)?;
		}
		if let Some(sbgp) = &self.sbgp {
			sbgp.mux(writer)?;
		}
		if let Some(subs) = &self.subs {
			subs.mux(writer)?;
		}
		for unknown in &self.unknown {
			unknown.mux(writer)?;
		}
		Ok(())
	}
}
