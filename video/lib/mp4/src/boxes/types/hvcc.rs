use std::io;

use bytes::Bytes;
use h265::HEVCDecoderConfigurationRecord;

use crate::boxes::header::BoxHeader;
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// HEVC (H.265) Configuration Box
/// ISO/IEC 14496-15:2022 - 8.4
pub struct HvcC {
	pub header: BoxHeader,
	pub hevc_config: HEVCDecoderConfigurationRecord,
}

impl HvcC {
	pub fn new(hevc_config: HEVCDecoderConfigurationRecord) -> Self {
		Self {
			header: BoxHeader::new(Self::NAME),
			hevc_config,
		}
	}
}

impl BoxType for HvcC {
	const NAME: [u8; 4] = *b"hvcC";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);
		Ok(Self {
			header,
			hevc_config: HEVCDecoderConfigurationRecord::demux(&mut reader)?,
		})
	}

	fn primitive_size(&self) -> u64 {
		self.hevc_config.size()
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.hevc_config.mux(writer)
	}
}
