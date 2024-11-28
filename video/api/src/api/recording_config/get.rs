use std::sync::Arc;

use pb::scuffle::video::v1::types::access_token_scope::Permission;
use pb::scuffle::video::v1::types::Resource;
use pb::scuffle::video::v1::{RecordingConfigGetRequest, RecordingConfigGetResponse};
use video_common::database::{AccessToken, DatabaseTable};

use crate::api::utils::{get, impl_request_scopes, ApiRequest, TonicRequest};
use crate::global::ApiGlobal;
use crate::ratelimit::RateLimitResource;

impl_request_scopes!(
	RecordingConfigGetRequest,
	video_common::database::RecordingConfig,
	(Resource::RecordingConfig, Permission::Read),
	RateLimitResource::RecordingConfigGet
);

pub fn build_query(
	req: &RecordingConfigGetRequest,
	access_token: &AccessToken,
) -> tonic::Result<utils::database::QueryBuilder<'static>> {
	let mut qb = scuffle_utils::database::QueryBuilder::default();
	qb.push("SELECT * FROM ")
		.push(<RecordingConfigGetRequest as TonicRequest>::Table::NAME)
		.push(" WHERE ");
	let mut seperated = qb.separated(" AND ");

	get::organization_id(&mut seperated, access_token.organization_id);
	get::ids(&mut seperated, &req.ids);
	get::search_options(&mut seperated, req.search_options.as_ref())?;

	Ok(qb)
}

impl ApiRequest<RecordingConfigGetResponse> for tonic::Request<RecordingConfigGetRequest> {
	async fn process<G: ApiGlobal>(
		&self,
		global: &Arc<G>,
		access_token: &AccessToken,
	) -> tonic::Result<tonic::Response<RecordingConfigGetResponse>> {
		let req = self.get_ref();

		let query = build_query(req, access_token)?;

		let results = query.build_query_as().fetch_all(global.db()).await.map_err(|err| {
			tracing::error!(err = %err, "failed to fetch {}s", <RecordingConfigGetRequest as TonicRequest>::Table::FRIENDLY_NAME);
			tonic::Status::internal(format!(
				"failed to fetch {}s",
				<RecordingConfigGetRequest as TonicRequest>::Table::FRIENDLY_NAME
			))
		})?;

		Ok(tonic::Response::new(RecordingConfigGetResponse {
			recording_configs: results
				.into_iter()
				.map(video_common::database::RecordingConfig::into_proto)
				.collect(),
		}))
	}
}
