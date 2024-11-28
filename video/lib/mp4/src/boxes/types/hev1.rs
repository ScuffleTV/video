use std::io;

use bytes::{Buf, Bytes};

use super::btrt::Btrt;
use super::hvcc::HvcC;
use super::stsd::{SampleEntry, VisualSampleEntry};
use crate::boxes::header::BoxHeader;
use crate::boxes::traits::BoxType;
use crate::boxes::DynBox;
use crate::codec::VideoCodec;

#[derive(Debug, Clone, PartialEq)]
/// HEVC (H.265) Codec Box
/// ISO/IEC 14496-15:2022 - 8.4
pub struct Hev1 {
	pub header: BoxHeader,
	pub visual_sample_entry: SampleEntry<VisualSampleEntry>,
	pub hvcc: HvcC,
	pub btrt: Option<Btrt>,
	pub unknown: Vec<DynBox>,
}

impl Hev1 {
	pub fn new(visual_sample_entry: SampleEntry<VisualSampleEntry>, hvcc: HvcC, btrt: Option<Btrt>) -> Self {
		Self {
			header: BoxHeader::new(Self::NAME),
			visual_sample_entry,
			hvcc,
			btrt,
			unknown: Vec::new(),
		}
	}

	pub fn codec(&self) -> io::Result<VideoCodec> {
		Ok(VideoCodec::Hevc {
			constraint_indicator: self.hvcc.hevc_config.general_constraint_indicator_flags,
			level: self.hvcc.hevc_config.general_level_idc,
			profile: self.hvcc.hevc_config.general_profile_idc,
			profile_compatibility: self.hvcc.hevc_config.general_profile_compatibility_flags,
			tier: self.hvcc.hevc_config.general_tier_flag,
			general_profile_space: self.hvcc.hevc_config.general_profile_space,
		})
	}
}

impl BoxType for Hev1 {
	const NAME: [u8; 4] = *b"hev1";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let mut visual_sample_entry = SampleEntry::<VisualSampleEntry>::demux(&mut reader)?;

		let mut hvcc = None;
		let mut btrt = None;
		let mut unknown = Vec::new();

		while reader.has_remaining() {
			let dyn_box = DynBox::demux(&mut reader)?;
			match dyn_box {
				DynBox::HvcC(b) => {
					hvcc = Some(b);
				}
				DynBox::Btrt(b) => {
					btrt = Some(b);
				}
				DynBox::Clap(b) => {
					visual_sample_entry.extension.clap = Some(b);
				}
				DynBox::Pasp(b) => {
					visual_sample_entry.extension.pasp = Some(b);
				}
				DynBox::Colr(b) => {
					visual_sample_entry.extension.colr = Some(b);
				}
				_ => {
					unknown.push(dyn_box);
				}
			}
		}

		let hvcc = hvcc.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "trak box is missing tkhd box"))?;

		Ok(Self {
			header,
			visual_sample_entry,
			hvcc,
			btrt,
			unknown,
		})
	}

	fn primitive_size(&self) -> u64 {
		self.visual_sample_entry.size()
			+ self.hvcc.size()
			+ self.btrt.as_ref().map(|b| b.size()).unwrap_or(0)
			+ self.unknown.iter().map(|b| b.size()).sum::<u64>()
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.visual_sample_entry.mux(writer)?;
		self.hvcc.mux(writer)?;
		if let Some(btrt) = &self.btrt {
			btrt.mux(writer)?;
		}
		for unknown in &self.unknown {
			unknown.mux(writer)?;
		}
		Ok(())
	}
}
