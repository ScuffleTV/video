use std::io;

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::{Buf, Bytes};

use super::decoder_config::DecoderConfigDescriptor;
use super::sl_config::SLConfigDescriptor;
use crate::boxes::types::esds::descriptor::header::{DescriptorHeader, DescriptorTag};
use crate::boxes::types::esds::descriptor::traits::DescriptorType;
use crate::boxes::types::esds::descriptor::DynDescriptor;

#[derive(Debug, Clone, PartialEq)]
/// ES Descriptor
/// ISO/IEC 14496-1:2010(E) - 7.2.6.5
pub struct EsDescriptor {
	pub header: DescriptorHeader,
	pub es_id: u16,
	pub stream_priority: u8, // 5 bits
	pub depends_on_es_id: Option<u16>,
	pub url: Option<String>,
	pub ocr_es_id: Option<u16>,
	pub decoder_config: Option<DecoderConfigDescriptor>,
	pub sl_config: Option<SLConfigDescriptor>,
	pub unknown: Vec<DynDescriptor>,
}

impl EsDescriptor {
	pub fn new(
		es_id: u16,
		stream_priority: u8,
		depends_on_es_id: Option<u16>,
		url: Option<String>,
		ocr_es_id: Option<u16>,
		decoder_config: Option<DecoderConfigDescriptor>,
		sl_config: Option<SLConfigDescriptor>,
	) -> Self {
		Self {
			header: DescriptorHeader::new(Self::TAG),
			es_id,
			stream_priority,
			depends_on_es_id,
			url,
			ocr_es_id,
			decoder_config,
			sl_config,
			unknown: Vec::new(),
		}
	}
}

impl DescriptorType for EsDescriptor {
	const TAG: DescriptorTag = DescriptorTag::ESDescrTag;

	fn demux(header: DescriptorHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let es_id = reader.read_u16::<BigEndian>()?;
		let flag = reader.read_u8()?;
		let stream_priority = flag & 0b00011111;
		let stream_dependence_flag = (flag & 0b10000000) != 0;
		let url_flag = (flag & 0b01000000) != 0;
		let ocr_stream_flag = (flag & 0b00100000) != 0;
		let depends_on_es_id = if stream_dependence_flag {
			Some(reader.read_u16::<BigEndian>()?)
		} else {
			None
		};

		let url = if url_flag {
			let size = reader.read_u8()?;
			let mut url = String::new();
			for _ in 0..size {
				url.push(reader.read_u8()? as char);
			}
			Some(url)
		} else {
			None
		};

		let ocr_es_id = if ocr_stream_flag {
			Some(reader.read_u16::<BigEndian>()?)
		} else {
			None
		};

		let mut decoder_config = None;
		let mut sl_config = None;
		let mut unknown = Vec::new();

		while reader.has_remaining() {
			let descriptor = DynDescriptor::demux(&mut reader)?;
			match descriptor {
				DynDescriptor::DecoderConfig(desc) => {
					decoder_config = Some(desc);
				}
				DynDescriptor::SLConfig(desc) => {
					sl_config = Some(desc);
				}
				_ => unknown.push(descriptor),
			}
		}

		Ok(Self {
			header,
			es_id,
			stream_priority,
			depends_on_es_id,
			url,
			ocr_es_id,
			decoder_config,
			sl_config,
			unknown,
		})
	}

	fn primitive_size(&self) -> u64 {
		2 // es_id
        + 1 // flag
        + if self.depends_on_es_id.is_some() { 2 } else { 0 }
        + if self.url.is_some() { 1 + self.url.as_ref().unwrap().len() as u64 } else { 0 }
        + if self.ocr_es_id.is_some() { 2 } else { 0 }
        + self.decoder_config.as_ref().map(|d| d.size()).unwrap_or(0)
        + self.sl_config.as_ref().map(|d| d.size()).unwrap_or(0)
        + self.unknown.iter().map(|d| d.size()).sum::<u64>()
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		writer.write_u16::<BigEndian>(self.es_id)?;
		let mut flag = self.stream_priority & 0b00011111;
		if self.depends_on_es_id.is_some() {
			flag |= 0b10000000;
		}
		if self.url.is_some() {
			flag |= 0b01000000;
		}
		if self.ocr_es_id.is_some() {
			flag |= 0b00100000;
		}
		writer.write_u8(flag)?;
		if let Some(depends_on_es_id) = self.depends_on_es_id {
			writer.write_u16::<BigEndian>(depends_on_es_id)?;
		}
		if let Some(url) = &self.url {
			writer.write_u8(url.len() as u8)?;
			for c in url.chars() {
				writer.write_u8(c as u8)?;
			}
		}
		if let Some(ocr_es_id) = self.ocr_es_id {
			writer.write_u16::<BigEndian>(ocr_es_id)?;
		}
		if let Some(decoder_config) = &self.decoder_config {
			decoder_config.mux(writer)?;
		}
		if let Some(sl_config) = &self.sl_config {
			sl_config.mux(writer)?;
		}
		for descriptor in &self.unknown {
			descriptor.mux(writer)?;
		}
		Ok(())
	}
}
