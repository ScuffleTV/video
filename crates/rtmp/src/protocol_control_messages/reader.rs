use std::io::Cursor;

use byteorder::{BigEndian, ReadBytesExt};

use super::errors::ProtocolControlMessageError;

pub struct ProtocolControlMessageReader;

impl ProtocolControlMessageReader {
    pub fn read_set_chunk_size(data: &[u8]) -> Result<u32, ProtocolControlMessageError> {
        let mut cursor = Cursor::new(data);
        let chunk_size = cursor.read_u32::<BigEndian>()?;
        Ok(chunk_size)
    }
}

#[cfg(test)]
#[cfg_attr(all(test, coverage_nightly), coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn test_reader_read_set_chunk_size() {
        let data = vec![0x00, 0x00, 0x00, 0x01];
        let chunk_size = ProtocolControlMessageReader::read_set_chunk_size(&data).unwrap();
        assert_eq!(chunk_size, 1);
    }
}
