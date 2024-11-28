use std::io;

use byteorder::WriteBytesExt;
use bytes::Bytes;

use crate::boxes::types::esds::descriptor::header::{DescriptorHeader, DescriptorTag};
use crate::boxes::types::esds::descriptor::traits::DescriptorType;

#[derive(Debug, Clone, PartialEq)]
/// SL Config Descriptor
/// ISO/IEC 14496-1:2010(E) - 7.2.6.8
pub struct SLConfigDescriptor {
	pub header: DescriptorHeader,
	pub predefined: u8,
	pub data: Bytes,
}

impl DescriptorType for SLConfigDescriptor {
	const TAG: DescriptorTag = DescriptorTag::SLConfigDescrTag;

	fn demux(header: DescriptorHeader, data: Bytes) -> io::Result<Self> {
		if data.is_empty() {
			return Err(io::Error::new(
				io::ErrorKind::InvalidData,
				"SLConfigDescriptor must have at least 1 byte",
			));
		}

		let predefined = data[0];
		let data = data.slice(1..);

		Ok(Self {
			header,
			predefined,
			data,
		})
	}

	fn primitive_size(&self) -> u64 {
		1 // predefined
        + self.data.len() as u64
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		writer.write_u8(self.predefined)?;
		writer.write_all(&self.data)?;

		Ok(())
	}
}
