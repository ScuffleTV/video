use std::io;

use av1::AV1CodecConfigurationRecord;
use bytes::Bytes;

use crate::boxes::header::BoxHeader;
use crate::boxes::traits::BoxType;

#[derive(Debug, Clone, PartialEq)]
/// AV1 Configuration Box
/// https://aomediacodec.github.io/av1-isobmff/#av1codecconfigurationbox-section
pub struct Av1C {
	pub header: BoxHeader,
	pub av1_config: AV1CodecConfigurationRecord,
}

impl Av1C {
	pub fn new(av1_config: AV1CodecConfigurationRecord) -> Self {
		Self {
			header: BoxHeader::new(Self::NAME),
			av1_config,
		}
	}
}

impl BoxType for Av1C {
	const NAME: [u8; 4] = *b"av1C";

	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self> {
		let mut reader = io::Cursor::new(data);
		Ok(Self {
			header,
			av1_config: AV1CodecConfigurationRecord::demux(&mut reader)?,
		})
	}

	fn primitive_size(&self) -> u64 {
		self.av1_config.size()
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.av1_config.mux(writer)
	}
}
