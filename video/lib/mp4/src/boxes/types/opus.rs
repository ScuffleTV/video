use std::io;

use bytes::{Buf, Bytes};

use super::btrt::Btrt;
use super::stsd::{AudioSampleEntry, SampleEntry};
use crate::boxes::header::BoxHeader;
use crate::boxes::traits::BoxType;
use crate::boxes::DynBox;
use crate::codec::AudioCodec;

#[derive(Debug, Clone, PartialEq)]
/// Opus Audio Sample Entry
/// Encapsulation of Opus in ISO Base Media File Format - Version 0.8.1
pub struct Opus {
	pub header: BoxHeader,
	pub audio_sample_entry: SampleEntry<AudioSampleEntry>,
	pub btrt: Option<Btrt>,
	pub unknown: Vec<DynBox>,
}

impl Opus {
	pub fn new(audio_sample_entry: SampleEntry<AudioSampleEntry>, btrt: Option<Btrt>) -> Self {
		Self {
			header: BoxHeader::new(Self::NAME),
			audio_sample_entry,
			btrt,
			unknown: Vec::new(),
		}
	}

	pub fn codec(&self) -> io::Result<AudioCodec> {
		Ok(AudioCodec::Opus)
	}
}

impl BoxType for Opus {
	const NAME: [u8; 4] = *b"Opus";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let audio_sample_entry = SampleEntry::<AudioSampleEntry>::demux(&mut reader)?;
		let mut btrt = None;
		let mut unknown = Vec::new();

		while reader.has_remaining() {
			let dyn_box = DynBox::demux(&mut reader)?;
			match dyn_box {
				DynBox::Btrt(btrt_box) => {
					btrt = Some(btrt_box);
				}
				_ => {
					unknown.push(dyn_box);
				}
			}
		}

		Ok(Self {
			header,
			audio_sample_entry,
			btrt,
			unknown,
		})
	}

	fn primitive_size(&self) -> u64 {
		self.audio_sample_entry.size()
			+ self.btrt.as_ref().map(|b| b.size()).unwrap_or(0)
			+ self.unknown.iter().map(|b| b.size()).sum::<u64>()
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.audio_sample_entry.mux(writer)?;
		if let Some(btrt) = &self.btrt {
			btrt.mux(writer)?;
		}
		for unknown in &self.unknown {
			unknown.mux(writer)?;
		}
		Ok(())
	}
}
