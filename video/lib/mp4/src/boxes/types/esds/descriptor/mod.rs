use std::io;

use byteorder::WriteBytesExt;
use bytes::Bytes;

use self::header::DescriptorHeader;
use self::traits::DescriptorType;
use self::types::decoder_config::DecoderConfigDescriptor;
use self::types::decoder_specific_info::DecoderSpecificInfoDescriptor;
use self::types::es::EsDescriptor;
use self::types::sl_config::SLConfigDescriptor;

pub mod header;
pub mod traits;
pub mod types;

#[derive(Debug, Clone, PartialEq)]
pub enum DynDescriptor {
	Es(EsDescriptor),
	DecoderConfig(DecoderConfigDescriptor),
	DecoderSpecificInfo(DecoderSpecificInfoDescriptor),
	SLConfig(SLConfigDescriptor),
	Unknown(DescriptorHeader, Bytes),
}

impl DynDescriptor {
	pub fn demux(reader: &mut io::Cursor<Bytes>) -> io::Result<Self> {
		let (header, data) = DescriptorHeader::parse(reader)?;
		match header.tag {
			EsDescriptor::TAG => Ok(Self::Es(EsDescriptor::demux(header, data)?)),
			DecoderConfigDescriptor::TAG => Ok(Self::DecoderConfig(DecoderConfigDescriptor::demux(header, data)?)),
			DecoderSpecificInfoDescriptor::TAG => {
				Ok(Self::DecoderSpecificInfo(DecoderSpecificInfoDescriptor::demux(header, data)?))
			}
			SLConfigDescriptor::TAG => Ok(Self::SLConfig(SLConfigDescriptor::demux(header, data)?)),
			_ => Ok(Self::Unknown(header, data)),
		}
	}

	pub fn size(&self) -> u64 {
		match self {
			Self::Es(desc) => desc.size(),
			Self::DecoderConfig(desc) => desc.size(),
			Self::DecoderSpecificInfo(desc) => desc.size(),
			Self::SLConfig(desc) => desc.size(),
			Self::Unknown(_, data) => {
				1 // tag
                + {
                    let mut size = data.len() as u32;
                    let mut bytes_required = 0;
                    loop {
                        size >>= 7;
                        bytes_required += 1;
                        if size == 0 {
                            break;
                        }
                    }

                    bytes_required // number of bytes required to encode the size
                }
                + data.len() as u64 // data
			}
		}
	}

	pub fn mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		match self {
			Self::Es(desc) => desc.mux(writer),
			Self::DecoderConfig(desc) => desc.mux(writer),
			Self::DecoderSpecificInfo(desc) => desc.mux(writer),
			Self::SLConfig(desc) => desc.mux(writer),
			Self::Unknown(header, data) => {
				writer.write_u8(header.tag.into())?;
				let mut size = data.len() as u32;
				loop {
					let byte = (size & 0b01111111) as u8;
					size >>= 7;
					if size == 0 {
						writer.write_u8(byte)?;
						break;
					} else {
						writer.write_u8(byte | 0b10000000)?;
					}
				}
				writer.write_all(data)?;

				Ok(())
			}
		}
	}
}
