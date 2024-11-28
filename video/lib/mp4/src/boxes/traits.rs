use std::io;

use byteorder::WriteBytesExt;
use bytes::Bytes;

use super::header::BoxHeader;

pub trait BoxType {
	const NAME: [u8; 4];

	/// Parse a box from a byte stream. The basic header is already parsed.
	fn demux(header: BoxHeader, data: Bytes) -> io::Result<Self>
	where
		Self: Sized;

	/// The size of the box without the basic header.
	fn primitive_size(&self) -> u64;

	/// Write the box to a byte stream. The basic header is already written.
	fn primitive_mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()>;

	/// Write the box to a byte stream.
	fn mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		self.validate()?;

		let size = self.size();
		if size > u32::MAX as u64 {
			writer.write_u32::<byteorder::BigEndian>(1)?;
		} else {
			writer.write_u32::<byteorder::BigEndian>(size as u32)?;
		}

		writer.write_all(&Self::NAME)?;

		if size > u32::MAX as u64 {
			writer.write_u64::<byteorder::BigEndian>(size)?;
		}

		self.primitive_mux(writer)
	}

	/// Size of the box including the basic header.
	fn size(&self) -> u64 {
		let primitive_size = self.primitive_size() + 8;

		if primitive_size > u32::MAX as u64 {
			primitive_size + 8
		} else {
			primitive_size
		}
	}

	/// Validate the box.
	fn validate(&self) -> io::Result<()> {
		Ok(())
	}
}
