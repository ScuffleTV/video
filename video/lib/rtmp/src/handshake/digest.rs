use bytes::Bytes;
use hmac::{Hmac, Mac};
use sha2::Sha256;

use super::define;
use super::define::SchemaVersion;
use super::errors::DigestError;

pub struct DigestProcessor {
	data: Bytes,
	key: Bytes,
}

impl DigestProcessor {
	pub fn new(data: Bytes, key: Bytes) -> Self {
		Self { data, key }
	}

	/// Read digest from message
	/// According the the spec the schema can either be in the order of
	///   time, version, key, digest (schema 0)
	/// or
	///   time, version, digest, key (schema 1)
	pub fn read_digest(&self) -> Result<(Bytes, SchemaVersion), DigestError> {
		if let Ok(digest) = self.generate_and_validate(SchemaVersion::Schema0) {
			Ok((digest, SchemaVersion::Schema0))
		} else {
			let digest = self.generate_and_validate(SchemaVersion::Schema1)?;
			Ok((digest, SchemaVersion::Schema1))
		}
	}

	pub fn generate_and_fill_digest(&self, version: SchemaVersion) -> Result<(Bytes, Bytes, Bytes), DigestError> {
		let (left_part, _, right_part) = self.cook_raw_message(version)?;
		let computed_digest = self.make_digest(&left_part, &right_part)?;

		// The reason we return 3 parts vs 1 is because if we return 1 part we need to
		// copy the memory But this is unnecessary because we are just going to write it
		// into a buffer.
		Ok((left_part, computed_digest, right_part))
	}

	fn find_digest_offset(&self, version: SchemaVersion) -> Result<usize, DigestError> {
		const OFFSET_LENGTH: usize = 4;

		// in schema 0 the digest is after the key (which is after the time and version)
		// in schema 1 the digest is after the time and version
		let schema_offset = match version {
			SchemaVersion::Schema0 => define::CHUNK_LENGTH + define::TIME_VERSION_LENGTH,
			SchemaVersion::Schema1 => define::TIME_VERSION_LENGTH,
		};

		// No idea why this isn't a be u32.
		// It seems to be 4 x 8bit values we add together.
		// We then mod it by the chunk length - digest length - offset length
		// Then add the schema offset and offset length to get the digest offset
		Ok((*self.data.get(schema_offset).unwrap() as usize
			+ *self.data.get(schema_offset + 1).unwrap() as usize
			+ *self.data.get(schema_offset + 2).unwrap() as usize
			+ *self.data.get(schema_offset + 3).unwrap() as usize)
			% (define::CHUNK_LENGTH - define::RTMP_DIGEST_LENGTH - OFFSET_LENGTH)
			+ schema_offset
			+ OFFSET_LENGTH)
	}

	fn cook_raw_message(&self, version: SchemaVersion) -> Result<(Bytes, Bytes, Bytes), DigestError> {
		let digest_offset = self.find_digest_offset(version)?;

		// We split the message into 3 parts:
		// 1. The part before the digest
		// 2. The digest
		// 3. The part after the digest
		// This is so we can calculate the digest.
		// We then compare it to the digest we read from the message.
		// If they are the same we have a valid message.

		// Slice is a O(1) operation and does not copy the memory.
		let left_part = self.data.slice(0..digest_offset);
		let digest_data = self.data.slice(digest_offset..digest_offset + define::RTMP_DIGEST_LENGTH);
		let right_part = self.data.slice(digest_offset + define::RTMP_DIGEST_LENGTH..);

		Ok((left_part, digest_data, right_part))
	}

	pub fn make_digest(&self, left: &[u8], right: &[u8]) -> Result<Bytes, DigestError> {
		// New hmac from the key
		let mut mac = Hmac::<Sha256>::new_from_slice(&self.key[..]).unwrap();
		// Update the hmac with the left and right parts
		mac.update(left);
		mac.update(right);

		// Finalize the hmac and get the digest
		let result = mac.finalize().into_bytes();
		if result.len() != define::RTMP_DIGEST_LENGTH {
			return Err(DigestError::DigestLengthNotCorrect);
		}

		// This does a copy of the memory but its only 32 bytes so its not a big deal.
		Ok(result.to_vec().into())
	}

	fn generate_and_validate(&self, version: SchemaVersion) -> Result<Bytes, DigestError> {
		// We need the 3 parts so we can calculate the digest and compare it to the
		// digest we read from the message.
		let (left_part, digest_data, right_part) = self.cook_raw_message(version)?;

		// If the digest we calculated is the same as the digest we read from the
		// message we have a valid message.
		if digest_data == self.make_digest(&left_part, &right_part)? {
			Ok(digest_data)
		} else {
			// This does not mean the message is invalid, it just means we need to try the
			// other schema. If both schemas fail then the message is invalid and its likely
			// a simple handshake.
			Err(DigestError::CannotGenerate)
		}
	}
}
