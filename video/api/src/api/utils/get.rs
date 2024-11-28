use pb::scuffle::video::v1::types::SearchOptions;
use tonic::Status;
use ulid::Ulid;

use super::tags::validate_tags;

pub fn organization_id(seperated: &mut scuffle_utils::database::Separated<'_, '_>, organization_id: Ulid) {
	seperated.push("organization_id = ");
	seperated.push_bind_unseparated(organization_id);
}

pub fn ids(seperated: &mut scuffle_utils::database::Separated<'_, '_>, ids: &[pb::scuffle::types::Ulid]) {
	if !ids.is_empty() {
		seperated.push("id = ANY(");
		seperated.push_bind_unseparated(
			ids.iter()
				.copied()
				.map(pb::scuffle::types::Ulid::into_ulid)
				.collect::<Vec<_>>(),
		);
		seperated.push_unseparated(")");
	}
}

pub fn search_options(
	seperated: &mut scuffle_utils::database::Separated<'_, '_>,
	search_options: Option<&SearchOptions>,
) -> tonic::Result<()> {
	if let Some(options) = search_options {
		if let Some(after_id) = options.after_id.as_ref() {
			if options.reverse {
				seperated.push("id < ");
			} else {
				seperated.push("id > ");
			}
			seperated.push_bind_unseparated(after_id.into_ulid());
		}

		validate_tags(options.tags.as_ref())?;

		if let Some(tags) = options.tags.as_ref() {
			if !tags.tags.is_empty() {
				seperated.push("tags @> ");
				seperated.push_bind_unseparated(
					serde_json::to_value(&tags.tags).map_err(|_| Status::internal("failed to serialize tags"))?,
				);
			}
		}

		let limit = if options.limit == 0 {
			100
		} else if options.limit >= 1 && options.limit <= 1000 {
			options.limit
		} else {
			return Err(Status::invalid_argument("limit must be between 1 and 1000"));
		} as i64;

		if options.reverse {
			seperated.push_unseparated(" ORDER BY id DESC");
		} else {
			seperated.push_unseparated(" ORDER BY id ASC");
		}

		seperated.push_unseparated(" LIMIT ");
		seperated.push_bind_unseparated(limit);
	} else {
		seperated.push_unseparated(" ORDER BY id ASC LIMIT 100");
	}

	Ok(())
}
