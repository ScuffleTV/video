use std::collections::HashMap;
use std::sync::Arc;

use pb::ext::UlidExt;
use pb::scuffle::video::v1::types::{access_token_scope, AccessTokenScope, Resource, SearchOptions, Tags};
use pb::scuffle::video::v1::{
	AccessTokenCreateRequest, AccessTokenCreateResponse, AccessTokenDeleteRequest, AccessTokenDeleteResponse,
	AccessTokenGetRequest, AccessTokenGetResponse, AccessTokenTagRequest, AccessTokenTagResponse, AccessTokenUntagRequest,
	AccessTokenUntagResponse,
};
use video_common::database::AccessToken;

use crate::api::access_token::{self, AccessTokenServer};
use crate::tests::api::utils::{assert_query_matches, process_request};
use crate::tests::global::GlobalState;
use crate::tests::utils;

#[tokio::test]
async fn test_access_token_get_qb() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let test_cases = vec![
		(
			AccessTokenGetRequest {
				ids: vec![access_token.id.into()],
				search_options: None,
			},
			Ok("SELECT * FROM access_tokens WHERE organization_id = $1 AND id = ANY($2) ORDER BY id ASC LIMIT 100"),
		),
		(
			AccessTokenGetRequest {
				ids: vec![access_token.id.into()],
				search_options: Some(SearchOptions {
					limit: 1,
					reverse: true,
					after_id: Some(access_token.id.into()),
					tags: None,
				}),
			},
			Ok(
				"SELECT * FROM access_tokens WHERE organization_id = $1 AND id = ANY($2) AND id < $3 ORDER BY id DESC LIMIT $4",
			),
		),
	];

	for (req, expected) in test_cases {
		let result = access_token::get::build_query(&req, &access_token);
		assert_query_matches(result, expected);
	}

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_access_token_create_qb() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let test_cases = vec![(
		AccessTokenCreateRequest {
			scopes: vec![AccessTokenScope {
				permission: vec![access_token_scope::Permission::Read.into()],
				resource: None,
			}],
			tags: None,
			..Default::default()
		},
		Ok(
			"INSERT INTO access_tokens (id,organization_id,secret_token,scopes,last_active_at,updated_at,expires_at,tags) VALUES ($1,$2,$3,$4,$5,$6,$7,$8) RETURNING *",
		),
	)];

	for (req, expected) in test_cases {
		let result = access_token::create::build_query(
			&req,
			&access_token,
			access_token::create::validate(&req, &access_token).unwrap(),
		);
		assert_query_matches(result, expected);
	}

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_access_token_tag_qb() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let test_cases = vec![(
		AccessTokenTagRequest {
			id: Some(access_token.id.into()),
			tags: Some(Tags {
				tags: vec![("example_tag".to_string(), "example_value".to_string())]
					.into_iter()
					.collect(),
			}),
		},
		Ok(
			"WITH mt AS (SELECT id, tags || $1 AS new_tags, CASE WHEN tags @> $1 THEN 1 WHEN COUNT(jsonb_object_keys(tags || $1)) > $2 THEN 2 ELSE 0 END AS status FROM access_tokens WHERE id = $3 AND organization_id = $4 GROUP BY id, organization_id) UPDATE access_tokens AS t SET tags = CASE WHEN mt.status = 0 THEN mt.new_tags ELSE tags END, updated_at = CASE WHEN mt.status = 0 THEN now() ELSE updated_at END FROM mt WHERE t.id = mt.id RETURNING t.tags as tags, mt.status as status;",
		),
	)];

	for (req, expected) in test_cases {
		assert!(access_token::tag::validate(&req).is_ok());
		let result = access_token::tag::build_query(&req, &access_token);
		assert_query_matches(result, expected);
	}

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_access_token_untag_qb() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let test_cases = vec![(
		AccessTokenUntagRequest {
			id: Some(access_token.id.into()),
			tags: vec!["example_tag".to_string()],
		},
		Ok(
			"WITH rt AS (SELECT id, tags - $1::TEXT[] AS new_tags, CASE WHEN NOT tags ?| $1 THEN 1 ELSE 0 END AS status FROM access_tokens WHERE id = $2 AND organization_id = $3 GROUP BY id, organization_id) UPDATE access_tokens AS t SET tags = CASE WHEN rt.status = 0 THEN rt.new_tags ELSE tags END, updated_at = CASE WHEN rt.status = 0 THEN now() ELSE updated_at END FROM rt WHERE t.id = rt.id RETURNING t.tags AS tags, rt.status AS status;",
		),
	)];

	for (req, expected) in test_cases {
		assert!(access_token::untag::validate(&req).is_ok());
		let result = access_token::untag::build_query(&req, &access_token);
		assert_query_matches(result, expected);
	}

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_access_token_tag() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let tag_request = AccessTokenTagRequest {
		id: Some(access_token.id.into()),
		tags: Some(Tags {
			tags: vec![("key".to_string(), "value".to_string())].into_iter().collect(),
		}),
	};

	let response: AccessTokenTagResponse = process_request(&global, &access_token, tag_request)
		.await
		.expect("Tagging should be successful");
	let tags = response.tags.unwrap();
	assert_eq!(tags.tags.get("key").unwrap(), &"value");

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_access_token_untag() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	// Tag the token first
	let tag_request = AccessTokenTagRequest {
		id: Some(access_token.id.into()),
		tags: Some(Tags {
			tags: vec![("key".to_string(), "value".to_string())].into_iter().collect(),
		}),
	};

	process_request::<_, AccessTokenTagResponse>(&global, &access_token, tag_request)
		.await
		.expect("Tagging should be successful");

	// Now, untag
	let untag_request = AccessTokenUntagRequest {
		id: Some(access_token.id.into()),
		tags: vec!["key".to_string()],
	};

	let response: AccessTokenUntagResponse = process_request(&global, &access_token, untag_request)
		.await
		.expect("Untagging should be successful");
	let tags = response.tags.unwrap();
	assert!(tags.tags.is_empty(), "Tags should be empty after untagging");

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_access_token_create() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	// Test case: Create a basic access token
	let req = AccessTokenCreateRequest {
		scopes: vec![AccessTokenScope {
			permission: vec![access_token_scope::Permission::Read.into()],
			resource: None,
		}],
		tags: None,
		..Default::default()
	};

	let response: AccessTokenCreateResponse = process_request(&global, &access_token, req).await.unwrap();
	assert!(!response.secret.is_empty(), "Secret token should not be empty");
	let created_token = response.access_token.as_ref().unwrap();
	assert!(
		created_token
			.scopes
			.iter()
			.any(|scope| scope.permission.contains(&access_token_scope::Permission::Read.into()))
	);
	assert!(created_token.tags.is_none() || created_token.tags.as_ref().unwrap().tags.is_empty());

	// Test case: Create an access token with specific tags
	let req = AccessTokenCreateRequest {
		scopes: vec![AccessTokenScope {
			permission: vec![access_token_scope::Permission::Write.into()],
			resource: Some(Resource::Event as i32),
		}],
		tags: Some(Tags {
			tags: vec![("tag_key".to_string(), "tag_value".to_string())].into_iter().collect(),
		}),
		..Default::default()
	};

	let response: AccessTokenCreateResponse = process_request(&global, &access_token, req).await.unwrap();
	let created_token_with_tags = response.access_token.as_ref().unwrap();
	assert!(!response.secret.is_empty(), "Secret token should not be empty");
	assert!(created_token_with_tags.scopes.iter().any(|scope| {
		scope.permission.contains(&access_token_scope::Permission::Write.into())
			&& scope.resource == Some(Resource::Event as i32)
	}));
	assert_eq!(
		created_token_with_tags.tags.as_ref().unwrap().tags.get("tag_key").unwrap(),
		"tag_value"
	);

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_access_token_get() {
	let (global, handler, main_access_token) = scuffle_utilssetup(Default::default()).await;

	// Create multiple access tokens with different tags for testing
	let created_tokens = vec![
		scuffle_utilscreate_access_token(
			&global,
			&main_access_token.organization_id,
			vec![],
			vec![
				("tag1".to_string(), "value1".to_string()),
				("common".to_string(), "shared".to_string()),
			]
			.into_iter()
			.collect(),
		)
		.await,
		scuffle_utilscreate_access_token(
			&global,
			&main_access_token.organization_id,
			vec![],
			vec![
				("tag2".to_string(), "value2".to_string()),
				("common".to_string(), "shared".to_string()),
			]
			.into_iter()
			.collect(),
		)
		.await,
	];

	// Fetch the created tokens using AccessTokenGetRequest
	let get_request = AccessTokenGetRequest {
		ids: created_tokens.iter().map(|token| token.id.into()).collect(),
		search_options: None,
	};

	let response: AccessTokenGetResponse = process_request(&global, &main_access_token, get_request).await.unwrap();
	let fetched_tokens = response.access_tokens;

	// Assertions
	assert_eq!(fetched_tokens.len(), created_tokens.len(), "Should fetch all created tokens");
	for token in fetched_tokens {
		let original_token = created_tokens
			.iter()
			.find(|&t| t.id == token.id.into_ulid())
			.expect("Fetched token must match one of the created ones");
		assert_eq!(
			token.tags.unwrap().tags,
			original_token.tags,
			"Tags should match for token ID"
		);
		// Add more assertions as needed to compare other fields
	}

	// Fetch tokens with a specific tag
	let tag_search_request = AccessTokenGetRequest {
		ids: vec![],
		search_options: Some(SearchOptions {
			limit: 2,
			reverse: false,
			after_id: None,
			tags: Some(Tags {
				tags: vec![("common".to_string(), "shared".to_string())].into_iter().collect(),
			}),
		}),
	};

	let response: AccessTokenGetResponse = process_request(&global, &main_access_token, tag_search_request)
		.await
		.unwrap();
	let fetched_tokens = response.access_tokens;

	// Assertions for tag-based search
	assert!(!fetched_tokens.is_empty(), "Should fetch tokens with specific tags");
	for token in &fetched_tokens {
		assert!(
			token.tags.as_ref().unwrap().tags.contains_key("common"),
			"Fetched tokens should contain the 'common' tag"
		);
	}

	// Fetch tokens with limit and reverse options
	let limited_request = AccessTokenGetRequest {
		ids: vec![],
		search_options: Some(SearchOptions {
			limit: 1,
			reverse: true,
			after_id: None,
			tags: None,
		}),
	};

	let limited_response: AccessTokenGetResponse =
		process_request(&global, &main_access_token, limited_request).await.unwrap();
	let limited_tokens = limited_response.access_tokens;

	// Assertions for limit and reverse options
	assert_eq!(limited_tokens.len(), 1, "Should fetch only one token due to limit");

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_access_token_delete() {
	let (global, handler, main_access_token) = scuffle_utilssetup(Default::default()).await;

	// Create access tokens to be deleted
	let token_to_delete =
		scuffle_utilscreate_access_token(&global, &main_access_token.organization_id, vec![], HashMap::new()).await;

	// Delete request with a token the caller should have permission to delete
	let delete_request = AccessTokenDeleteRequest {
		ids: vec![token_to_delete.id.into()],
	};

	let delete_response: AccessTokenDeleteResponse =
		process_request(&global, &main_access_token, delete_request).await.unwrap();
	let deleted_tokens = delete_response.ids;
	let failed_deletions = delete_response.failed_deletes;

	// Assertions for successful deletion
	assert_eq!(deleted_tokens.len(), 1, "Should successfully delete one token");
	assert!(
		deleted_tokens.contains(&token_to_delete.id.into()),
		"Deleted token list should contain the token ID"
	);
	assert!(failed_deletions.is_empty(), "No deletions should fail in this scenario");

	// Attempt to delete the caller's own token
	let self_delete_request = AccessTokenDeleteRequest {
		ids: vec![main_access_token.id.into()],
	};

	let self_delete_response: AccessTokenDeleteResponse = process_request(&global, &main_access_token, self_delete_request)
		.await
		.unwrap();

	// Assertions for deletion attempt on own token
	assert!(self_delete_response.ids.is_empty(), "Should not delete own token");
	assert_eq!(
		self_delete_response.failed_deletes.len(),
		1,
		"Should fail to delete own token"
	);
	assert_eq!(
		self_delete_response.failed_deletes[0].id,
		Some(main_access_token.id.into()),
		"Failed deletion should be for own token"
	);
	assert_eq!(
		self_delete_response.failed_deletes[0].reason, "cannot delete own access token",
		"Failed deletion reason should be correct"
	);

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_access_token_boiler_plate() {
	let (global, handler, main_access_token) = scuffle_utilssetup(Default::default()).await;

	let no_scopes_token =
		scuffle_utilscreate_access_token(&global, &main_access_token.organization_id, vec![], HashMap::new()).await;

	let server = AccessTokenServer::<GlobalState>::new();

	use pb::scuffle::video::v1::access_token_server::AccessToken as _;

	fn build_request<T>(global: &Arc<GlobalState>, token: &AccessToken, req: T) -> tonic::Request<T> {
		let mut req = tonic::Request::new(req);

		req.extensions_mut().insert(token.clone());
		req.extensions_mut().insert(global.clone());

		req
	}

	let response = server
		.get(build_request(
			&global,
			&main_access_token,
			AccessTokenGetRequest {
				ids: vec![main_access_token.id.into()],
				search_options: None,
			},
		))
		.await
		.unwrap();
	assert_eq!(response.get_ref().access_tokens.len(), 1);
	assert_eq!(
		response.metadata().get("x-ratelimit-reset").unwrap(),
		"30",
		"rate limit reset should be 30 seconds"
	);
	assert_eq!(
		response.metadata().get("x-ratelimit-remaining").unwrap(),
		"990",
		"rate limit remaining should be 990"
	);
	assert_eq!(
		response.metadata().len(),
		2,
		"rate limit headers should be the only headers set"
	);

	let response = server
		.get(build_request(&global, &no_scopes_token, AccessTokenGetRequest::default()))
		.await
		.unwrap_err();
	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: access_token:read");

	let response = server
		.create(build_request(
			&global,
			&main_access_token,
			AccessTokenCreateRequest {
				scopes: vec![AccessTokenScope {
					permission: vec![access_token_scope::Permission::Read.into()],
					resource: None,
				}],
				expires_at: None,
				tags: None,
			},
		))
		.await
		.unwrap();
	assert!(response.get_ref().access_token.is_some());
	assert_eq!(
		response.metadata().get("x-ratelimit-reset").unwrap(),
		"30",
		"rate limit reset should be 30 seconds"
	);
	assert_eq!(
		response.metadata().get("x-ratelimit-remaining").unwrap(),
		"990",
		"rate limit remaining should be 990"
	);
	assert_eq!(
		response.metadata().len(),
		2,
		"rate limit headers should be the only headers set"
	);

	let response = server
		.create(build_request(&global, &no_scopes_token, AccessTokenCreateRequest::default()))
		.await
		.unwrap_err();
	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: access_token:create");

	let response = server
		.tag(build_request(
			&global,
			&main_access_token,
			AccessTokenTagRequest {
				id: Some(main_access_token.id.into()),
				tags: Some(Tags {
					tags: vec![("key".to_string(), "value".to_string())].into_iter().collect(),
				}),
			},
		))
		.await
		.unwrap();
	assert!(response.get_ref().tags.is_some());
	assert_eq!(
		response.metadata().get("x-ratelimit-reset").unwrap(),
		"30",
		"rate limit reset should be 30 seconds"
	);
	assert_eq!(
		response.metadata().get("x-ratelimit-remaining").unwrap(),
		"990",
		"rate limit remaining should be 990"
	);

	let response = server
		.tag(build_request(&global, &no_scopes_token, AccessTokenTagRequest::default()))
		.await
		.unwrap_err();
	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: access_token:modify");

	let response = server
		.untag(build_request(
			&global,
			&main_access_token,
			AccessTokenUntagRequest {
				id: Some(main_access_token.id.into()),
				tags: vec!["key".to_string()],
			},
		))
		.await
		.unwrap();
	assert!(response.get_ref().tags.is_some());
	assert_eq!(
		response.metadata().get("x-ratelimit-reset").unwrap(),
		"30",
		"rate limit reset should be 30 seconds"
	);
	assert_eq!(
		response.metadata().get("x-ratelimit-remaining").unwrap(),
		"990",
		"rate limit remaining should be 990"
	);

	let response = server
		.untag(build_request(&global, &no_scopes_token, AccessTokenUntagRequest::default()))
		.await
		.unwrap_err();
	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: access_token:modify");

	let response = server
		.delete(build_request(
			&global,
			&main_access_token,
			AccessTokenDeleteRequest {
				ids: vec![main_access_token.id.into()],
			},
		))
		.await
		.unwrap();

	assert_eq!(response.get_ref().failed_deletes.len(), 1); // Cannot delete own token
	assert_eq!(
		response.metadata().get("x-ratelimit-reset").unwrap(),
		"30",
		"rate limit reset should be 30 seconds"
	);
	assert_eq!(
		response.metadata().get("x-ratelimit-remaining").unwrap(),
		"990",
		"rate limit remaining should be 990"
	);

	let response = server
		.delete(build_request(&global, &no_scopes_token, AccessTokenDeleteRequest::default()))
		.await
		.unwrap_err();
	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: access_token:delete");

	scuffle_utilsteardown(global, handler).await;
}
