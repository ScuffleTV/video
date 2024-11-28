use std::io;

use bytes::{Buf, Bytes};

use self::descriptor::traits::DescriptorType;
use self::descriptor::types::es::EsDescriptor;
use self::descriptor::DynDescriptor;
use crate::boxes::header::{BoxHeader, FullBoxHeader};
use crate::boxes::traits::BoxType;

pub mod descriptor;

#[derive(Debug, Clone, PartialEq)]
/// Elementary Stream Descriptor Box
/// ISO/IEC 14496-14:2020(E) - 6.7.2
pub struct Esds {
	pub header: FullBoxHeader,
	pub es_descriptor: EsDescriptor,
	pub unknown: Vec<DynDescriptor>,
}

impl Esds {
	pub fn new(es_descriptor: EsDescriptor) -> Self {
		Self {
			header: FullBoxHeader::new(Self::NAME, 0, 0),
			es_descriptor,
			unknown: Vec::new(),
		}
	}
}

impl BoxType for Esds {
	const NAME: [u8; 4] = *b"esds";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);

		let header = FullBoxHeader::demux(header, &mut reader)?;

		let mut es_descriptor = None;
		let mut unknown = Vec::new();

		while reader.has_remaining() {
			let descriptor = DynDescriptor::demux(&mut reader)?;
			match descriptor {
				DynDescriptor::Es(desc) => {
					es_descriptor = Some(desc);
				}
				_ => unknown.push(descriptor),
			}
		}

		let es_descriptor = es_descriptor
			.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "esds box must contain es descriptor"))?;

		Ok(Self {
			header,
			es_descriptor,
			unknown,
		})
	}

	fn primitive_size(&self) -> u64 {
		self.header.size() + self.es_descriptor.size()
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.header.mux(writer)?;

		self.es_descriptor.mux(writer)?;

		Ok(())
	}
}
