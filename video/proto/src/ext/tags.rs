use std::collections::HashMap;

use crate::scuffle::video::v1::types::Tags;

impl From<HashMap<String, String>> for Tags {
	fn from(map: HashMap<String, String>) -> Self {
		Self { tags: map }
	}
}

impl From<Tags> for HashMap<String, String> {
	fn from(tags: Tags) -> Self {
		tags.tags
	}
}
