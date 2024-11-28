use std::io;

use byteorder::WriteBytesExt;
use bytes::Bytes;

use super::header::DescriptorTag;
use super::DescriptorHeader;

pub trait DescriptorType {
	const TAG: DescriptorTag;

	fn demux(header: DescriptorHeader, data: Bytes) -> io::Result<Self>
	where
		Self: Sized;

	fn primitive_size(&self) -> u64;

	fn size(&self) -> u64 {
		let primitive_size = self.primitive_size();

		primitive_size // size of the primitive data
        + 1 // tag
        + {
            let mut size = primitive_size as u32;
            let mut bytes_required = 0;
            loop {
                size >>= 7;
                bytes_required += 1;
                if size == 0 {
                    break;
                }
            }

            bytes_required // number of bytes required to encode the size
        } as u64
	}

	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()>;

	fn mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		writer.write_u8(Self::TAG.into())?;
		let size = self.primitive_size() as u32;
		let mut size = size;
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
		self.primitive_mux(writer)?;

		Ok(())
	}
}
