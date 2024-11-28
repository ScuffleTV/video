use std::io;

use bytesio::bit_reader::BitReader;
use bytesio::bit_writer::BitWriter;

pub fn read_exp_golomb(reader: &mut BitReader) -> io::Result<u64> {
	let mut leading_zeros = 0;
	while !reader.read_bit()? {
		leading_zeros += 1;
	}

	let mut result = 1;
	for _ in 0..leading_zeros {
		result <<= 1;
		result |= reader.read_bit()? as u64;
	}

	Ok(result - 1)
}

pub fn read_signed_exp_golomb(reader: &mut BitReader) -> io::Result<i64> {
	let exp_glob = read_exp_golomb(reader)?;

	if exp_glob % 2 == 0 {
		Ok(-((exp_glob / 2) as i64))
	} else {
		Ok((exp_glob / 2) as i64 + 1)
	}
}

pub fn write_exp_golomb(writer: &mut BitWriter, input: u64) -> io::Result<()> {
	let mut number = input + 1;
	let mut leading_zeros = 0;
	while number > 1 {
		number >>= 1;
		leading_zeros += 1;
	}

	for _ in 0..leading_zeros {
		writer.write_bit(false)?;
	}

	writer.write_bits(input + 1, leading_zeros + 1)?;

	Ok(())
}

pub fn write_signed_exp_golomb(writer: &mut BitWriter, number: i64) -> io::Result<()> {
	let number = if number <= 0 {
		-number as u64 * 2
	} else {
		number as u64 * 2 - 1
	};

	write_exp_golomb(writer, number)
}

#[cfg(test)]
mod tests;
