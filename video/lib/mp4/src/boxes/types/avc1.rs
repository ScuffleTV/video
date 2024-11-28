use std::io;

use bytes::{Buf, Bytes};

use super::avcc::AvcC;
use super::btrt::Btrt;
use super::stsd::{SampleEntry, VisualSampleEntry};
use crate::boxes::header::BoxHeader;
use crate::boxes::traits::BoxType;
use crate::boxes::DynBox;
use crate::codec::VideoCodec;

#[derive(Debug, Clone, PartialEq)]
/// AVC Codec Box
/// ISO/IEC 14496-15:2022(E) - 6.5.3
pub struct Avc1 {
	pub header: BoxHeader,
	pub visual_sample_entry: SampleEntry<VisualSampleEntry>,
	pub avcc: AvcC,
	pub btrt: Option<Btrt>,
	pub unknown: Vec<DynBox>,
}

impl Avc1 {
	pub fn new(visual_sample_entry: SampleEntry<VisualSampleEntry>, avcc: AvcC, btrt: Option<Btrt>) -> Self {
		Self {
			header: BoxHeader::new(Self::NAME),
			visual_sample_entry,
			avcc,
			btrt,
			unknown: Vec::new(),
		}
	}

	pub fn codec(&self) -> io::Result<VideoCodec> {
		Ok(VideoCodec::Avc {
			constraint_set: self.avcc.avc_decoder_configuration_record.profile_compatibility,
			level: self.avcc.avc_decoder_configuration_record.level_indication,
			profile: self.avcc.avc_decoder_configuration_record.profile_indication,
		})
	}
}

impl BoxType for Avc1 {
	const NAME: [u8; 4] = *b"avc1";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let mut visual_sample_entry = SampleEntry::<VisualSampleEntry>::demux(&mut reader)?;

		let mut avcc = None;
		let mut btrt = None;
		let mut unknown = Vec::new();

		while reader.has_remaining() {
			let dyn_box = DynBox::demux(&mut reader)?;
			match dyn_box {
				DynBox::AvcC(b) => {
					avcc = Some(b);
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

		let avcc = avcc.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "trak box is missing tkhd box"))?;

		Ok(Self {
			header,
			visual_sample_entry,
			avcc,
			btrt,
			unknown,
		})
	}

	fn primitive_size(&self) -> u64 {
		self.visual_sample_entry.size()
			+ self.avcc.size()
			+ self.btrt.as_ref().map(|b| b.size()).unwrap_or(0)
			+ self.unknown.iter().map(|b| b.size()).sum::<u64>()
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.visual_sample_entry.mux(writer)?;
		self.avcc.mux(writer)?;
		if let Some(btrt) = &self.btrt {
			btrt.mux(writer)?;
		}
		for unknown in &self.unknown {
			unknown.mux(writer)?;
		}
		Ok(())
	}
}
