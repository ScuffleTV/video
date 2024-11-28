use std::sync::Arc;

use pb::scuffle::video::v1::types::access_token_scope::Permission;
use pb::scuffle::video::v1::types::Resource;
use pb::scuffle::video::v1::{AccessTokenGetRequest, AccessTokenGetResponse};
use tonic::Status;
use video_common::database::{AccessToken, DatabaseTable};

use crate::api::utils::{get, impl_request_scopes, ApiRequest, TonicRequest};
use crate::global::ApiGlobal;
use crate::ratelimit::RateLimitResource;

impl_request_scopes!(
	AccessTokenGetRequest,
	video_common::database::AccessToken,
	(Resource::AccessToken, Permission::Read),
	RateLimitResource::AccessTokenGet
);

pub fn build_query(
	req: &AccessTokenGetRequest,
	access_token: &AccessToken,
) -> tonic::Result<utils::database::QueryBuilder<'static>> {
	let mut qb = scuffle_utils::database::QueryBuilder::default();
	qb.push("SELECT * FROM ")
		.push(<AccessTokenGetRequest as TonicRequest>::Table::NAME)
		.push(" WHERE ");

	let mut seperated = qb.separated(" AND ");

	get::organization_id(&mut seperated, access_token.organization_id);
	get::ids(&mut seperated, &req.ids);
	get::search_options(&mut seperated, req.search_options.as_ref())?;

	Ok(qb)
}

impl ApiRequest<AccessTokenGetResponse> for tonic::Request<AccessTokenGetRequest> {
	async fn process<G: ApiGlobal>(
		&self,
		global: &Arc<G>,
		access_token: &AccessToken,
	) -> tonic::Result<tonic::Response<AccessTokenGetResponse>> {
		let req = self.get_ref();

		let query = build_query(req, access_token)?;

		let access_tokens = query
			.build_query_as()
			.fetch_all(global.db())
			.await
			.map_err(|err| {
				tracing::error!(err = %err, "failed to fetch {}s", <AccessTokenGetRequest as TonicRequest>::Table::FRIENDLY_NAME);
				Status::internal(format!(
					"failed to fetch {}s",
					<AccessTokenGetRequest as TonicRequest>::Table::FRIENDLY_NAME
				))
			})?
			.into_iter()
			.map(AccessToken::into_proto)
			.collect();

		Ok(tonic::Response::new(AccessTokenGetResponse { access_tokens }))
	}
}
