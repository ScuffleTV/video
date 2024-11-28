use std::collections::HashMap;

use pb::scuffle::video::v1::types::Tags;
use tonic::Status;
use ulid::Ulid;
use video_common::database::DatabaseTable;

const MAX_TAG_COUNT: usize = 10;
const MAX_TAG_KEY_LENGTH: usize = 16;
const MAX_TAG_VALUE_LENGTH: usize = 32;
const TAG_ALPHABET: &str = r#"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-+?'";:[]{}_"#;

pub fn validate_tags(tags: Option<&Tags>) -> tonic::Result<()> {
	if let Some(tags) = tags {
		if tags.tags.len() > MAX_TAG_COUNT {
			return Err(Status::invalid_argument(format!("too many tags, max {MAX_TAG_COUNT}")));
		}

		for (key, value) in &tags.tags {
			if key.len() > MAX_TAG_KEY_LENGTH {
				return Err(Status::invalid_argument(format!(
					"tag key too long, max length {MAX_TAG_KEY_LENGTH} characters"
				)));
			}

			if value.len() > MAX_TAG_VALUE_LENGTH {
				return Err(Status::invalid_argument(format!(
					"tag value too long, max length {MAX_TAG_VALUE_LENGTH} characters",
				)));
			}

			if let Some(c) = key.chars().find(|c| !TAG_ALPHABET.contains(*c)) {
				return Err(Status::invalid_argument(format!(
					"tag key contains invalid characters: {key}: {c}"
				)));
			}

			if let Some(c) = value.chars().find(|c| !TAG_ALPHABET.contains(*c)) {
				return Err(Status::invalid_argument(format!(
					"tag value contains invalid characters: {value}: {c}"
				)));
			}
		}
	}

	Ok(())
}

pub fn validate_tags_array(tags: &[String]) -> tonic::Result<()> {
	if tags.len() > MAX_TAG_COUNT {
		return Err(Status::invalid_argument(format!("too many tags, max {MAX_TAG_COUNT}")));
	}

	for tag in tags {
		if tag.len() > MAX_TAG_KEY_LENGTH {
			return Err(Status::invalid_argument(format!(
				"tag key too long, max length {MAX_TAG_KEY_LENGTH} characters"
			)));
		}

		if !tag.chars().all(|c| TAG_ALPHABET.contains(c)) {
			return Err(Status::invalid_argument("tag key contains invalid characters"));
		}
	}

	Ok(())
}

#[derive(postgres_from_row::FromRow)]
pub struct TagExt {
	pub tags: scuffle_utils::database::Json<HashMap<String, String>>,
	pub status: i64,
}

impl TagExt {
	pub fn into_tags(self) -> tonic::Result<Tags> {
		match self.status {
			0 | 1 => {}
			2 => {
				return Err(Status::invalid_argument(
					"too many tags, cannot add tag(s) to exceed max tag count",
				));
			}
			_ => {
				return Err(Status::internal(format!(
					"invalid status code returned from query: {}",
					self.status
				)));
			}
		}

		Ok(Tags { tags: self.tags.0 })
	}
}

pub fn add_tag_query<D: DatabaseTable>(
	tags: &HashMap<String, String>,
	id: Ulid,
	organization_id: Option<Ulid>,
) -> scuffle_utils::database::QueryBuilder<'_> {
	let mut qb = scuffle_utils::database::QueryBuilder::default();

	qb.push("WITH mt AS (SELECT id, tags || ")
		.push_bind(utils::database::Json(tags))
		.push(" AS new_tags, CASE WHEN tags @> $1 THEN 1 WHEN COUNT(jsonb_object_keys(tags || $1)) > ")
		.push_bind(MAX_TAG_COUNT as i64)
		.push(" THEN 2 ELSE 0 END AS status FROM ")
		.push(D::NAME)
		.push(" WHERE id = ")
		.push_bind(id);

	if let Some(organization_id) = organization_id {
		qb.push(" AND organization_id = ").push_bind(organization_id);
	}

	qb.push(" GROUP BY id, organization_id) UPDATE ")
		.push(D::NAME)
		.push(" AS t SET tags = CASE WHEN mt.status = 0 THEN mt.new_tags ELSE tags END,")
		.push(" updated_at = CASE WHEN mt.status = 0 THEN now() ELSE updated_at END")
		.push(" FROM mt WHERE t.id = mt.id RETURNING t.tags as tags, mt.status as status;");

	qb
}

pub fn remove_tag_query<D: DatabaseTable>(
	tags: &[String],
	id: Ulid,
	organization_id: Option<Ulid>,
) -> scuffle_utils::database::QueryBuilder<'_> {
	let mut qb = scuffle_utils::database::QueryBuilder::default();

	qb.push("WITH rt AS (SELECT id, tags - ")
		.push_bind(tags)
		.push("::TEXT[] AS new_tags, CASE WHEN NOT tags ?| $1 THEN 1 ELSE 0 END AS status FROM ")
		.push(D::NAME)
		.push(" WHERE id = ")
		.push_bind(id);

	if let Some(organization_id) = organization_id {
		qb.push(" AND organization_id = ").push_bind(organization_id);
	}

	qb.push(" GROUP BY id, organization_id) UPDATE ")
		.push(D::NAME)
		.push(" AS t")
		.push(" SET tags = CASE WHEN rt.status = 0 THEN rt.new_tags ELSE tags END,")
		.push(" updated_at = CASE WHEN rt.status = 0 THEN now() ELSE updated_at END")
		.push(" FROM rt WHERE t.id = rt.id RETURNING t.tags AS tags, rt.status AS status;");

	qb
}

macro_rules! impl_tag_req {
	($req:ty, $resp:ident, $event_target:expr, [$id:ident] $event:expr) => {
		pub fn validate(req: &$req) -> tonic::Result<()> {
			crate::api::utils::tags::validate_tags(req.tags.as_ref())
		}

		pub fn build_query<'a>(req: &'a $req, access_token: &video_common::database::AccessToken) -> tonic::Result<utils::database::QueryBuilder<'a>> {
			let tags = req.tags.as_ref().ok_or_else(|| {
				tonic::Status::invalid_argument("tags must be provided to add a tag")
			})?;

			Ok(crate::api::utils::tags::add_tag_query::<<$req as crate::api::utils::TonicRequest>::Table>(&tags.tags, pb::ext::UlidExt::into_ulid(req.id), Some(access_token.organization_id)))
		}

		impl crate::api::utils::ApiRequest<$resp> for tonic::Request<$req> {
			async fn process<G: crate::global::ApiGlobal>(&self, global: &std::sync::Arc<G>, access_token: &video_common::database::AccessToken) -> tonic::Result<tonic::Response<$resp>> {
				let req = self.get_ref();

				validate(req)?;

				let query = build_query(req, access_token)?;

				let result: crate::api::utils::tags::TagExt = query.build_query_as().fetch_one(global.db()).await.map_err(|err| {
					tracing::error!(err = %err, "failed to update {}", <<$req as crate::api::utils::TonicRequest>::Table as video_common::database::DatabaseTable>::FRIENDLY_NAME);
					tonic::Status::internal(format!("failed to update {}", <<$req as crate::api::utils::TonicRequest>::Table as video_common::database::DatabaseTable>::FRIENDLY_NAME))
				})?;

				let $id = pb::ext::UlidExt::into_ulid(req.id);
				video_common::events::emit(global.nats(), &global.config().events.stream_name, access_token.organization_id, $event_target, $event).await;

				Ok(tonic::Response::new($resp {
					tags: Some(result.into_tags()?)
				}))
			}
		}
	};
}

pub(crate) use impl_tag_req;

macro_rules! impl_untag_req {
	($req:ty, $resp:ident, $event_target:expr, [$id:ident] $event:expr) => {
		pub fn validate(req: &$req) -> tonic::Result<()> {
			if req.tags.is_empty() {
				return Err(tonic::Status::invalid_argument("tags must be provided to remove a tag"));
			}
			crate::api::utils::tags::validate_tags_array(&req.tags)
		}

		pub fn build_query<'a>(req: &'a $req, access_token: &video_common::database::AccessToken) -> tonic::Result<utils::database::QueryBuilder<'a>> {
			Ok(crate::api::utils::tags::remove_tag_query::<<$req as crate::api::utils::TonicRequest>::Table>(&req.tags, pb::ext::UlidExt::into_ulid(req.id), Some(access_token.organization_id)))
		}

		impl crate::api::utils::ApiRequest<$resp> for tonic::Request<$req> {
			async fn process<G: crate::global::ApiGlobal>(&self, global: &std::sync::Arc<G>, access_token: &video_common::database::AccessToken) -> tonic::Result<tonic::Response<$resp>> {
				let req = self.get_ref();

				validate(req)?;

				let query = build_query(req, access_token)?;

				let result: crate::api::utils::tags::TagExt = query.build_query_as().fetch_one(global.db()).await.map_err(|err| {
					tracing::error!(err = %err, "failed to update {}", <<$req as crate::api::utils::TonicRequest>::Table as video_common::database::DatabaseTable>::FRIENDLY_NAME);
					tonic::Status::internal(format!("failed to update {}", <<$req as crate::api::utils::TonicRequest>::Table as video_common::database::DatabaseTable>::FRIENDLY_NAME))
				})?;

				let $id = pb::ext::UlidExt::into_ulid(req.id);
				video_common::events::emit(global.nats(), &global.config().events.stream_name, access_token.organization_id, $event_target, $event).await;

				Ok(tonic::Response::new($resp {
					tags: Some(result.into_tags()?)
				}))
			}
		}
	};
}

pub(crate) use impl_untag_req;
