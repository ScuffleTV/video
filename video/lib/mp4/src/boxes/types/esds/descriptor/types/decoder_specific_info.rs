use std::io;

use bytes::Bytes;

use crate::boxes::types::esds::descriptor::header::{DescriptorHeader, DescriptorTag};
use crate::boxes::types::esds::descriptor::traits::DescriptorType;

#[derive(Debug, Clone, PartialEq)]
/// Decoder Specific Info Descriptor
/// ISO/IEC 14496-1:2010(E) - 7.2.6.7
pub struct DecoderSpecificInfoDescriptor {
	pub header: DescriptorHeader,
	pub data: Bytes,
}

impl DescriptorType for DecoderSpecificInfoDescriptor {
	const TAG: DescriptorTag = DescriptorTag::DecSpecificInfoTag;

	fn demux(header: DescriptorHeader, data: Bytes) -> io::Result<Self> {
		Ok(Self { header, data })
	}

	fn primitive_size(&self) -> u64 {
		self.data.len() as u64
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		writer.write_all(&self.data)?;

		Ok(())
	}
}
