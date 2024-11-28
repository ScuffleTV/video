use std::io;

use bytes::{Buf, Bytes};

use super::sbgp::Sbgp;
use super::subs::Subs;
use super::tfdt::Tfdt;
use super::tfhd::Tfhd;
use super::trun::Trun;
use crate::boxes::header::BoxHeader;
use crate::boxes::traits::BoxType;
use crate::boxes::DynBox;

#[derive(Debug, Clone, PartialEq)]
/// Track Fragment Box
/// ISO/IEC 14496-12:2022(E) - 8.8.6
pub struct Traf {
	pub header: BoxHeader,
	pub tfhd: Tfhd,
	pub trun: Option<Trun>,
	pub sbgp: Option<Sbgp>,
	pub subs: Option<Subs>,
	pub tfdt: Option<Tfdt>,
	pub unknown: Vec<DynBox>,
}

impl Traf {
	pub fn new(tfhd: Tfhd, trun: Option<Trun>, tfdt: Option<Tfdt>) -> Self {
		Self {
			header: BoxHeader::new(Self::NAME),
			tfhd,
			trun,
			sbgp: None,
			subs: None,
			tfdt,
			unknown: Vec::new(),
		}
	}

	/// This function will try to optimize the trun samples by using default
	/// values from the tfhd box.
	pub fn optimize(&mut self) {
		let Some(trun) = &mut self.trun else {
			return;
		};

		if trun.samples.is_empty() {
			return;
		}

		let tfhd = &mut self.tfhd;

		if tfhd.default_sample_flags.is_none()
			&& trun.samples.len() > 1
			&& trun.samples.iter().skip(2).all(|s| s.flags == trun.samples[1].flags)
		{
			tfhd.default_sample_flags = trun.samples[1].flags;

			let first_sample = trun.samples.first().unwrap();
			if first_sample.flags != tfhd.default_sample_flags {
				trun.first_sample_flags = first_sample.flags;
			}

			trun.samples.iter_mut().for_each(|s| s.flags = None);
		}

		if trun.samples.iter().all(|s| s.composition_time_offset == Some(0)) {
			trun.samples.iter_mut().for_each(|s| s.composition_time_offset = None);
		}

		if tfhd.default_sample_duration.is_none()
			&& trun.samples.iter().skip(1).all(|s| s.duration == trun.samples[0].duration)
		{
			tfhd.default_sample_duration = trun.samples[0].duration;
			trun.samples.iter_mut().for_each(|s| s.duration = None);
		}

		if tfhd.default_sample_size.is_none() && trun.samples.iter().skip(1).all(|s| s.size == trun.samples[0].size) {
			tfhd.default_sample_size = trun.samples[0].size;
			trun.samples.iter_mut().for_each(|s| s.size = None);
		}

		trun.header.flags = if trun.data_offset.is_some() {
			Trun::FLAG_DATA_OFFSET
		} else {
			0
		} | if trun.first_sample_flags.is_some() {
			Trun::FLAG_FIRST_SAMPLE_FLAGS
		} else {
			0
		} | if trun.samples.first().and_then(|s| s.duration).is_some() {
			Trun::FLAG_SAMPLE_DURATION
		} else {
			0
		} | if trun.samples.first().and_then(|s| s.size).is_some() {
			Trun::FLAG_SAMPLE_SIZE
		} else {
			0
		} | if trun.samples.first().and_then(|s| s.flags).is_some() {
			Trun::FLAG_SAMPLE_FLAGS
		} else {
			0
		} | if trun.samples.first().and_then(|s| s.composition_time_offset).is_some() {
			Trun::FLAG_SAMPLE_COMPOSITION_TIME_OFFSET
		} else {
			0
		};

		tfhd.header.flags = if tfhd.base_data_offset.is_some() {
			Tfhd::BASE_DATA_OFFSET_FLAG
		} else {
			0
		} | if tfhd.default_sample_duration.is_some() {
			Tfhd::DEFAULT_SAMPLE_DURATION_FLAG
		} else {
			0
		} | if tfhd.default_sample_flags.is_some() {
			Tfhd::DEFAULT_SAMPLE_FLAGS_FLAG
		} else {
			0
		} | if tfhd.default_sample_size.is_some() {
			Tfhd::DEFAULT_SAMPLE_SIZE_FLAG
		} else {
			0
		} | tfhd.header.flags & Tfhd::DEFAULT_BASE_IS_MOOF_FLAG;
	}

	pub fn duration(&self) -> u32 {
		let tfhd = &self.tfhd;
		let trun = &self.trun;

		if let Some(trun) = trun {
			let mut duration = 0;
			for sample in &trun.samples {
				if let Some(d) = sample.duration {
					duration += d;
				} else if let Some(d) = tfhd.default_sample_duration {
					duration += d;
				}
			}

			return duration;
		}

		0
	}

	pub fn contains_keyframe(&self) -> bool {
		let tfhd = &self.tfhd;
		let trun = &self.trun;

		if let Some(trun) = trun {
			if let Some(flags) = trun.first_sample_flags {
				if flags.sample_depends_on == 2 {
					return true;
				}
			}

			for sample in &trun.samples {
				if let Some(flags) = sample.flags {
					if flags.sample_depends_on == 2 {
						return true;
					}
				} else if let Some(flags) = tfhd.default_sample_flags {
					if flags.sample_depends_on == 2 {
						return true;
					}
				}
			}
		}

		false
	}
}

impl BoxType for Traf {
	const NAME: [u8; 4] = *b"traf";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let mut tfhd = None;
		let mut trun = None;
		let mut sbgp = None;
		let mut subs = None;
		let mut tfdt = None;

		let mut unknown = Vec::new();

		while reader.has_remaining() {
			let box_ = DynBox::demux(&mut reader)?;
			match box_ {
				DynBox::Tfhd(b) => {
					tfhd = Some(b);
				}
				DynBox::Trun(b) => {
					trun = Some(b);
				}
				DynBox::Sbgp(b) => {
					sbgp = Some(b);
				}
				DynBox::Subs(b) => {
					subs = Some(b);
				}
				DynBox::Tfdt(b) => {
					tfdt = Some(b);
				}
				_ => unknown.push(box_),
			}
		}

		let tfhd = tfhd.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "traf box must contain tfhd box"))?;

		Ok(Self {
			header,
			tfhd,
			trun,
			sbgp,
			subs,
			tfdt,
			unknown,
		})
	}

	fn primitive_size(&self) -> u64 {
		self.tfhd.size()
			+ self.trun.as_ref().map(|box_| box_.size()).unwrap_or(0)
			+ self.sbgp.as_ref().map(|box_| box_.size()).unwrap_or(0)
			+ self.subs.as_ref().map(|box_| box_.size()).unwrap_or(0)
			+ self.tfdt.as_ref().map(|box_| box_.size()).unwrap_or(0)
			+ self.unknown.iter().map(|box_| box_.size()).sum::<u64>()
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.tfhd.mux(writer)?;

		if let Some(box_) = &self.sbgp {
			box_.mux(writer)?;
		}

		if let Some(box_) = &self.subs {
			box_.mux(writer)?;
		}

		if let Some(box_) = &self.tfdt {
			box_.mux(writer)?;
		}

		for box_ in &self.unknown {
			box_.mux(writer)?;
		}

		if let Some(box_) = &self.trun {
			box_.mux(writer)?;
		}

		Ok(())
	}
}
