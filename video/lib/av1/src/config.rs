use std::io;

use bytes::Bytes;
use bytesio::bit_reader::BitReader;
use bytesio::bit_writer::BitWriter;
use bytesio::bytes_reader::BytesCursor;

#[derive(Debug, Clone, PartialEq)]
/// AV1 Codec Configuration Record
/// https://aomediacodec.github.io/av1-isobmff/#av1codecconfigurationbox-syntax
pub struct AV1CodecConfigurationRecord {
	pub marker: bool,
	pub version: u8,
	pub seq_profile: u8,
	pub seq_level_idx_0: u8,
	pub seq_tier_0: bool,
	pub high_bitdepth: bool,
	pub twelve_bit: bool,
	pub monochrome: bool,
	pub chroma_subsampling_x: bool,
	pub chroma_subsampling_y: bool,
	pub chroma_sample_position: u8,
	pub initial_presentation_delay_minus_one: Option<u8>,
	pub config_obu: Bytes,
}

impl AV1CodecConfigurationRecord {
	pub fn demux(reader: &mut io::Cursor<Bytes>) -> io::Result<Self> {
		let mut bit_reader = BitReader::new(reader);

		let marker = bit_reader.read_bit()?;
		let version = bit_reader.read_bits(7)? as u8;

		let seq_profile = bit_reader.read_bits(3)? as u8;
		let seq_level_idx_0 = bit_reader.read_bits(5)? as u8;

		let seq_tier_0 = bit_reader.read_bit()?;
		let high_bitdepth = bit_reader.read_bit()?;
		let twelve_bit = bit_reader.read_bit()?;
		let monochrome = bit_reader.read_bit()?;
		let chroma_subsampling_x = bit_reader.read_bit()?;
		let chroma_subsampling_y = bit_reader.read_bit()?;
		let chroma_sample_position = bit_reader.read_bits(2)? as u8;

		bit_reader.seek_bits(3)?; // reserved 3 bits

		let initial_presentation_delay_minus_one = if bit_reader.read_bit()? {
			Some(bit_reader.read_bits(4)? as u8)
		} else {
			bit_reader.seek_bits(4)?; // reserved 4 bits
			None
		};

		if !bit_reader.is_aligned() {
			return Err(io::Error::new(io::ErrorKind::InvalidData, "Bit reader is not aligned"));
		}

		let reader = bit_reader.into_inner();

		Ok(AV1CodecConfigurationRecord {
			marker,
			version,
			seq_profile,
			seq_level_idx_0,
			seq_tier_0,
			high_bitdepth,
			twelve_bit,
			monochrome,
			chroma_subsampling_x,
			chroma_subsampling_y,
			chroma_sample_position,
			initial_presentation_delay_minus_one,
			config_obu: reader.extract_remaining(),
		})
	}

	pub fn size(&self) -> u64 {
		1 // marker, version
        + 1 // seq_profile, seq_level_idx_0
        + 1 // seq_tier_0, high_bitdepth, twelve_bit, monochrome, chroma_subsampling_x, chroma_subsampling_y, chroma_sample_position
        + 1 // reserved, initial_presentation_delay_present, initial_presentation_delay_minus_one/reserved
        + self.config_obu.len() as u64
	}

	pub fn mux<T: io::Write>(&self, writer: &mut T) -> io::Result<()> {
		let mut bit_writer = BitWriter::default();

		bit_writer.write_bit(self.marker)?;
		bit_writer.write_bits(self.version as u64, 7)?;

		bit_writer.write_bits(self.seq_profile as u64, 3)?;
		bit_writer.write_bits(self.seq_level_idx_0 as u64, 5)?;

		bit_writer.write_bit(self.seq_tier_0)?;
		bit_writer.write_bit(self.high_bitdepth)?;
		bit_writer.write_bit(self.twelve_bit)?;
		bit_writer.write_bit(self.monochrome)?;
		bit_writer.write_bit(self.chroma_subsampling_x)?;
		bit_writer.write_bit(self.chroma_subsampling_y)?;
		bit_writer.write_bits(self.chroma_sample_position as u64, 2)?;

		bit_writer.write_bits(0, 3)?; // reserved 3 bits

		if let Some(initial_presentation_delay_minus_one) = self.initial_presentation_delay_minus_one {
			bit_writer.write_bit(true)?;
			bit_writer.write_bits(initial_presentation_delay_minus_one as u64, 4)?;
		} else {
			bit_writer.write_bit(false)?;
			bit_writer.write_bits(0, 4)?; // reserved 4 bits
		}

		writer.write_all(&bit_writer.into_inner())?;
		writer.write_all(&self.config_obu)?;

		Ok(())
	}
}
