use std::sync::Arc;

use chrono::TimeZone;
use pb::scuffle::video::v1::events_fetch_request::Target;
use pb::scuffle::video::v1::types::access_token_scope::Permission;
use pb::scuffle::video::v1::types::{event, Resource};
use pb::scuffle::video::v1::{AccessTokenCreateRequest, AccessTokenCreateResponse};
use tonic::Status;
use ulid::Ulid;
use video_common::database::{AccessToken, DatabaseTable};

use crate::api::utils::tags::validate_tags;
use crate::api::utils::{impl_request_scopes, AccessTokenExt, ApiRequest, RequiredScope, TonicRequest};
use crate::global::ApiGlobal;
use crate::ratelimit::RateLimitResource;

impl_request_scopes!(
	AccessTokenCreateRequest,
	video_common::database::AccessToken,
	(Resource::AccessToken, Permission::Create),
	RateLimitResource::AccessTokenCreate
);

pub fn validate(req: &AccessTokenCreateRequest, access_token: &AccessToken) -> tonic::Result<RequiredScope> {
	if req.scopes.len() > 20 {
		return Err(Status::invalid_argument("scopes must not be longer than 20"));
	}

	if req.scopes.iter().any(|s| s.permission.len() > 20) {
		return Err(Status::invalid_argument("permission must not be longer than 20"));
	}

	let permissions = RequiredScope(req.scopes.clone()).optimize();

	if req.scopes.is_empty() {
		return Err(Status::invalid_argument("scopes must not be empty"));
	}

	access_token.has_scope(&permissions)?;

	validate_tags(req.tags.as_ref())?;

	Ok(permissions)
}

pub fn build_query(
	req: &AccessTokenCreateRequest,
	access_token: &AccessToken,
	permissions: RequiredScope,
) -> tonic::Result<utils::database::QueryBuilder<'static>> {
	let mut qb = scuffle_utils::database::QueryBuilder::default();

	qb.push("INSERT INTO ")
		.push(<AccessTokenCreateRequest as TonicRequest>::Table::NAME)
		.push(" (");

	let mut seperated = qb.separated(",");

	seperated.push("id");
	seperated.push("organization_id");
	seperated.push("secret_token");
	seperated.push("scopes");
	seperated.push("last_active_at");
	seperated.push("updated_at");
	seperated.push("expires_at");
	seperated.push("tags");

	qb.push(") VALUES (");

	let mut seperated = qb.separated(",");

	seperated.push_bind(Ulid::new());
	seperated.push_bind(access_token.organization_id);
	seperated.push_bind(Ulid::new());
	seperated.push_bind(permissions.0.into_iter().map(utils::database::Protobuf).collect::<Vec<_>>());
	seperated.push_bind(None::<chrono::DateTime<chrono::Utc>>);
	seperated.push_bind(chrono::Utc::now());
	seperated.push_bind(req.expires_at.map(|x| chrono::Utc.timestamp_opt(x, 0).unwrap()));
	seperated.push_bind(utils::database::Json(req.tags.clone().unwrap_or_default().tags));

	qb.push(") RETURNING *");

	Ok(qb)
}

impl ApiRequest<AccessTokenCreateResponse> for tonic::Request<AccessTokenCreateRequest> {
	async fn process<G: ApiGlobal>(
		&self,
		global: &Arc<G>,
		access_token: &AccessToken,
	) -> tonic::Result<tonic::Response<AccessTokenCreateResponse>> {
		let req = self.get_ref();

		let permissions = validate(req, access_token)?;

		let query_builder = build_query(req, access_token, permissions)?;

		let result: <AccessTokenCreateRequest as TonicRequest>::Table =
			query_builder.build_query_as().fetch_one(global.db()).await.map_err(|err| {
				tracing::error!(err = %err, "failed to fetch {}s", <AccessTokenCreateRequest as TonicRequest>::Table::FRIENDLY_NAME);
				Status::internal(format!(
					"failed to fetch {}s",
					<AccessTokenCreateRequest as TonicRequest>::Table::FRIENDLY_NAME
				))
			})?;

		video_common::events::emit(
			global.nats(),
			&global.config().events.stream_name,
			access_token.organization_id,
			Target::AccessToken,
			event::Event::AccessToken(event::AccessToken {
				access_token_id: Some(result.id.into()),
				event: Some(event::access_token::Event::Created(event::access_token::Created {})),
			}),
		)
		.await;

		Ok(tonic::Response::new(AccessTokenCreateResponse {
			secret: result.secret_token.to_string(),
			access_token: Some(result.into_proto()),
		}))
	}
}
