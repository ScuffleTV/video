use std::io;

use bytes::{Buf, Bytes};

use super::btrt::Btrt;
use super::esds::Esds;
use super::stsd::{AudioSampleEntry, SampleEntry};
use crate::boxes::header::BoxHeader;
use crate::boxes::traits::BoxType;
use crate::boxes::DynBox;
use crate::codec::AudioCodec;

#[derive(Debug, Clone, PartialEq)]
/// AAC Audio Sample Entry
/// ISO/IEC 14496-14:2020(E) - 6.7
pub struct Mp4a {
	pub header: BoxHeader,
	pub audio_sample_entry: SampleEntry<AudioSampleEntry>,
	pub esds: Esds,
	pub btrt: Option<Btrt>,
	pub unknown: Vec<DynBox>,
}

impl Mp4a {
	pub fn new(audio_sample_entry: SampleEntry<AudioSampleEntry>, esds: Esds, btrt: Option<Btrt>) -> Self {
		Self {
			header: BoxHeader::new(Self::NAME),
			audio_sample_entry,
			esds,
			btrt,
			unknown: Vec::new(),
		}
	}

	#[allow(clippy::useless_asref)]
	pub fn codec(&self) -> io::Result<AudioCodec> {
		let info = self
			.esds
			.es_descriptor
			.decoder_config
			.as_ref()
			.and_then(|c| c.decoder_specific_info.as_ref().map(|c| c.data.clone()))
			.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Missing decoder specific info"))?;
		let aac_config = aac::AudioSpecificConfig::parse(info)?;

		Ok(AudioCodec::Aac {
			object_type: aac_config.audio_object_type,
		})
	}
}

impl BoxType for Mp4a {
	const NAME: [u8; 4] = *b"mp4a";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let audio_sample_entry = SampleEntry::<AudioSampleEntry>::demux(&mut reader)?;
		let mut btrt = None;
		let mut esds = None;
		let mut unknown = Vec::new();

		while reader.has_remaining() {
			let dyn_box = DynBox::demux(&mut reader)?;
			match dyn_box {
				DynBox::Btrt(btrt_box) => {
					btrt = Some(btrt_box);
				}
				DynBox::Esds(esds_box) => {
					esds = Some(esds_box);
				}
				_ => {
					unknown.push(dyn_box);
				}
			}
		}

		let esds = esds.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Missing esds box"))?;

		Ok(Self {
			header,
			audio_sample_entry,
			esds,
			btrt,
			unknown,
		})
	}

	fn primitive_size(&self) -> u64 {
		self.audio_sample_entry.size()
			+ self.btrt.as_ref().map(|b| b.size()).unwrap_or(0)
			+ self.esds.size()
			+ self.unknown.iter().map(|b| b.size()).sum::<u64>()
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.audio_sample_entry.mux(writer)?;
		self.esds.mux(writer)?;
		if let Some(btrt) = &self.btrt {
			btrt.mux(writer)?;
		}
		for unknown in &self.unknown {
			unknown.mux(writer)?;
		}
		Ok(())
	}
}
