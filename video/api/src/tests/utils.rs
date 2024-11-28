use std::collections::HashMap;
use std::ops::Add;
use std::sync::Arc;
use std::time::Duration;

use pb::scuffle::video::v1::types::{access_token_scope, AccessTokenScope};
use scuffle_utils::context::Handler;
use scuffle_utils::prelude::FutureTimeout;
use ulid::Ulid;
use video_common::database::AccessToken;

use super::global::{mock_global_state, GlobalState};
use crate::config::ApiConfig;
use crate::global::ApiGlobal;

pub async fn create_organization(global: &Arc<impl ApiGlobal>) -> video_common::database::Organization {
	scuffle_utils::database::query(
		"INSERT INTO organizations (id, name, updated_at, tags) VALUES ($1, $2, $3, $4) RETURNING *",
	)
	.bind(Ulid::new())
	.bind("test")
	.bind(chrono::Utc::now())
	.bind(utils::database::Json(std::collections::HashMap::<String, String>::default()))
	.build_query_as()
	.fetch_one(global.db())
	.await
	.unwrap()
}

pub async fn create_access_token(
	global: &Arc<impl ApiGlobal>,
	organization_id: &Ulid,
	scopes: Vec<utils::database::Protobuf<AccessTokenScope>>,
	tags: std::collections::HashMap<String, String>,
) -> video_common::database::AccessToken {
	scuffle_utils::database::query("INSERT INTO access_tokens (id, organization_id, secret_token, last_active_at, updated_at, expires_at, scopes, tags) VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING *")
		.bind(Ulid::new())
		.bind(organization_id)
		.bind(Ulid::new())
		.bind(chrono::Utc::now())
		.bind(chrono::Utc::now())
		.bind(chrono::Utc::now().add(chrono::Duration::days(1)))
		.bind(scopes)
		.bind(utils::database::Json(tags))
		.build_query_as().fetch_one(global.db()).await.unwrap()
}

// Shared setup function
pub async fn setup(config: ApiConfig) -> (Arc<GlobalState>, Handler, AccessToken) {
	let (global, handler) = mock_global_state(config).await;
	let org = create_organization(&global).await;
	let access_token = create_access_token(
		&global,
		&org.id,
		vec![utils::database::Protobuf(AccessTokenScope {
			permission: vec![access_token_scope::Permission::Admin.into()],
			resource: None,
		})],
		HashMap::new(),
	)
	.await;
	(global, handler, access_token)
}

// Shared teardown function
pub async fn teardown(global: Arc<GlobalState>, handler: Handler) {
	drop(global);
	handler
		.cancel()
		.timeout(Duration::from_secs(2))
		.await
		.expect("Failed to cancel handler");
}
