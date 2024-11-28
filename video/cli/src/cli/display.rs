use std::collections::HashMap;

use pb::ext::UlidExt;
use ulid::Ulid;

#[derive(Debug, serde::Serialize)]
pub struct DeleteResponse {
	pub ids: Vec<Ulid>,
	pub failed: Vec<DeleteResponseFailed>,
}

#[derive(Debug, serde::Serialize)]
pub struct DeleteResponseFailed {
	pub id: Ulid,
	pub error: String,
}

impl From<pb::scuffle::video::v1::types::FailedResource> for DeleteResponseFailed {
	fn from(failed: pb::scuffle::video::v1::types::FailedResource) -> Self {
		Self {
			id: failed.id.into_ulid(),
			error: failed.reason,
		}
	}
}

#[derive(Debug, serde::Serialize)]
pub struct TagResponse {
	pub id: Ulid,
	pub tags: HashMap<String, String>,
}
