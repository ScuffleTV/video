use std::collections::HashMap;
use std::sync::Arc;

use pb::ext::UlidExt;
use pb::scuffle::video::v1::types::{SearchOptions, Tags};
use pb::scuffle::video::v1::{
	S3BucketCreateRequest, S3BucketCreateResponse, S3BucketDeleteRequest, S3BucketDeleteResponse, S3BucketGetRequest,
	S3BucketGetResponse, S3BucketModifyRequest, S3BucketModifyResponse, S3BucketTagRequest, S3BucketTagResponse,
	S3BucketUntagRequest, S3BucketUntagResponse,
};
use video_common::database::AccessToken;

use crate::api::s3_bucket::{self, S3BucketServer};
use crate::tests::api::utils::{assert_query_matches, create_s3_bucket, process_request};
use crate::tests::global::GlobalState;
use crate::tests::utils;

#[tokio::test]
async fn test_s3_bucket_get_qb() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let test_cases = vec![
		(
			S3BucketGetRequest {
				ids: vec![access_token.organization_id.into()],
				search_options: None,
			},
			Ok("SELECT * FROM s3_buckets WHERE organization_id = $1 AND id = ANY($2) ORDER BY id ASC LIMIT 100"),
		),
		(
			S3BucketGetRequest {
				ids: vec![],
				search_options: Some(SearchOptions {
					limit: 10,
					reverse: true,
					after_id: Some(access_token.organization_id.into()),
					tags: Some(Tags {
						tags: vec![("example_tag".to_string(), "example_value".to_string())]
							.into_iter()
							.collect(),
					}),
				}),
			},
			Ok("SELECT * FROM s3_buckets WHERE organization_id = $1 AND id < $2 AND tags @> $3 ORDER BY id DESC LIMIT $4"),
		),
	];

	for (req, expected) in test_cases {
		let result = s3_bucket::get::build_query(&req, &access_token);
		assert_query_matches(result, expected);
	}

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_s3_bucket_create_qb() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let test_cases = vec![(
		S3BucketCreateRequest {
			tags: None,
			access_key_id: "access_key_id".to_string(),
			name: "name".to_string(),
			region: "us-east-1".to_string(),
			secret_access_key: "secret_access_key".to_string(),
			endpoint: None,
			public_url: None,
		},
		Ok(
			"INSERT INTO s3_buckets (id,organization_id,name,region,endpoint,access_key_id,secret_access_key,public_url,tags,managed) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) RETURNING *",
		),
	)];

	for (req, expected) in test_cases {
		assert!(s3_bucket::create::validate(&req).is_ok());
		let result = s3_bucket::create::build_query(&req, &access_token);
		assert_query_matches(result, expected);
	}

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_s3_bucket_modify_qb() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let test_cases = vec![
		(
			S3BucketModifyRequest {
				id: Some(access_token.id.into()),
				..Default::default()
			},
			Err("at least one field must be set to modify"),
		),
		(
			S3BucketModifyRequest {
				id: Some(access_token.id.into()),
				tags: Some(Tags {
					tags: vec![("example_tag".to_string(), "example_value".to_string())]
						.into_iter()
						.collect(),
				}),
				access_key_id: Some("example_access_key_id".to_string()),
				endpoint: Some("https://example_endpoint.com".to_string()),
				name: Some("example_name".to_string()),
				public_url: Some("https://example_public_url.com".to_string()),
				region: Some("us-east-1".to_string()),
				secret_access_key: Some("example_secret_access_key".to_string()),
			},
			Ok(
				"UPDATE s3_buckets SET access_key_id = $1,secret_access_key = $2,name = $3,region = $4,endpoint = $5,public_url = $6,tags = $7,updated_at = NOW() WHERE id = $8 AND organization_id = $9 RETURNING *",
			),
		),
	];

	for (req, expected) in test_cases {
		let result = s3_bucket::modify::build_query(&req, &access_token);
		assert_query_matches(result, expected);
	}

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_s3_bucket_tag_qb() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let test_cases = vec![(
		S3BucketTagRequest {
			id: Some(access_token.id.into()),
			tags: Some(Tags {
				tags: vec![("example_tag".to_string(), "example_value".to_string())]
					.into_iter()
					.collect(),
			}),
		},
		Ok(
			"WITH mt AS (SELECT id, tags || $1 AS new_tags, CASE WHEN tags @> $1 THEN 1 WHEN COUNT(jsonb_object_keys(tags || $1)) > $2 THEN 2 ELSE 0 END AS status FROM s3_buckets WHERE id = $3 AND organization_id = $4 GROUP BY id, organization_id) UPDATE s3_buckets AS t SET tags = CASE WHEN mt.status = 0 THEN mt.new_tags ELSE tags END, updated_at = CASE WHEN mt.status = 0 THEN now() ELSE updated_at END FROM mt WHERE t.id = mt.id RETURNING t.tags as tags, mt.status as status;",
		),
	)];

	for (req, expected) in test_cases {
		assert!(s3_bucket::tag::validate(&req).is_ok());
		let result = s3_bucket::tag::build_query(&req, &access_token);
		assert_query_matches(result, expected);
	}

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_s3_bucket_untag_qb() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let test_cases = vec![(
		S3BucketUntagRequest {
			id: Some(access_token.id.into()),
			tags: vec!["example_tag".to_string()],
		},
		Ok(
			"WITH rt AS (SELECT id, tags - $1::TEXT[] AS new_tags, CASE WHEN NOT tags ?| $1 THEN 1 ELSE 0 END AS status FROM s3_buckets WHERE id = $2 AND organization_id = $3 GROUP BY id, organization_id) UPDATE s3_buckets AS t SET tags = CASE WHEN rt.status = 0 THEN rt.new_tags ELSE tags END, updated_at = CASE WHEN rt.status = 0 THEN now() ELSE updated_at END FROM rt WHERE t.id = rt.id RETURNING t.tags AS tags, rt.status AS status;",
		),
	)];

	for (req, expected) in test_cases {
		assert!(s3_bucket::untag::validate(&req).is_ok());
		let result = s3_bucket::untag::build_query(&req, &access_token);
		assert_query_matches(result, expected);
	}

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_s3_bucket_tag() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let s3_bucket = create_s3_bucket(
		&global,
		access_token.organization_id,
		vec![("key".into(), "value".into())].into_iter().collect(),
	)
	.await;

	let response: S3BucketTagResponse = process_request(
		&global,
		&access_token,
		S3BucketTagRequest {
			id: Some(s3_bucket.id.into()),
			tags: Some(Tags {
				tags: vec![("key2".to_string(), "value2".to_string())].into_iter().collect(),
			}),
		},
	)
	.await
	.expect("Tagging should be successful");
	let tags = response.tags.unwrap();

	assert_eq!(tags.tags.get("key").unwrap(), &"value");
	assert_eq!(tags.tags.get("key2").unwrap(), &"value2");

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_s3_bucket_untag() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let s3_bucket = create_s3_bucket(
		&global,
		access_token.organization_id,
		vec![("key".into(), "value".into()), ("key2".into(), "value2".into())]
			.into_iter()
			.collect(),
	)
	.await;

	let response: S3BucketUntagResponse = process_request(
		&global,
		&access_token,
		S3BucketUntagRequest {
			id: Some(s3_bucket.id.into()),
			tags: vec!["key".to_string()],
		},
	)
	.await
	.expect("Untagging should be successful");
	let tags = response.tags.unwrap();
	assert_eq!(tags.tags.len(), 1, "Only 1 tag should be left");
	assert_eq!(tags.tags.get("key2").unwrap(), &"value2");

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_s3_bucket_create() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let response: S3BucketCreateResponse = process_request(
		&global,
		&access_token,
		S3BucketCreateRequest {
			access_key_id: "access_key_id".to_string(),
			name: "name".to_string(),
			region: "us-east-1".to_string(),
			secret_access_key: "secret_access_key".to_string(),
			endpoint: Some("https://endpoint.com".to_string()),
			public_url: Some("https://public_url.com".to_string()),
			tags: None,
		},
	)
	.await
	.unwrap();
	let created = response.s3_bucket.as_ref().unwrap();
	assert!(created.tags.is_none() || created.tags.as_ref().unwrap().tags.is_empty());

	let response: S3BucketCreateResponse = process_request(
		&global,
		&access_token,
		S3BucketCreateRequest {
			access_key_id: "access_key_id".to_string(),
			name: "name".to_string(),
			region: "us-east-1".to_string(),
			secret_access_key: "secret_access_key".to_string(),
			endpoint: None,
			public_url: None,
			tags: Some(Tags {
				tags: vec![("tag_key".to_string(), "tag_value".to_string())].into_iter().collect(),
			}),
		},
	)
	.await
	.unwrap();
	let created = response.s3_bucket.as_ref().unwrap();
	assert_eq!(created.tags.as_ref().unwrap().tags.get("tag_key").unwrap(), "tag_value");
	assert_eq!(created.tags.as_ref().unwrap().tags.len(), 1, "1 tag");
	assert_eq!(created.access_key_id, "access_key_id");
	assert_eq!(created.name, "name");
	assert_eq!(created.region, "us-east-1");
	assert_eq!(created.endpoint, None);
	assert_eq!(created.public_url, None);

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_s3_bucket_modify() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let s3_bucket = create_s3_bucket(&global, access_token.organization_id, HashMap::new()).await;

	let response: S3BucketModifyResponse = process_request(
		&global,
		&access_token,
		S3BucketModifyRequest {
			id: Some(s3_bucket.id.into()),
			tags: Some(Tags {
				tags: vec![("key3".to_string(), "value3".to_string())].into_iter().collect(),
			}),
			..Default::default()
		},
	)
	.await
	.unwrap();
	let created = response.s3_bucket.as_ref().unwrap();

	assert_eq!(
		created.tags.as_ref().unwrap().tags,
		vec![("key3".to_string(), "value3".to_string()),].into_iter().collect(),
		"tags changed"
	);

	let response: S3BucketModifyResponse = process_request(
		&global,
		&access_token,
		S3BucketModifyRequest {
			id: Some(s3_bucket.id.into()),
			tags: Some(Tags {
				tags: vec![("key4".to_string(), "value4".to_string())].into_iter().collect(),
			}),
			access_key_id: Some("access_key_id".to_string()),
			name: Some("name".to_string()),
			endpoint: Some("https://endpoint.com".to_string()),
			public_url: Some("https://public_url.com".to_string()),
			region: Some("us-east-1".to_string()),
			secret_access_key: Some("secret_access_key".to_string()),
		},
	)
	.await
	.unwrap();
	let created = response.s3_bucket.as_ref().unwrap();

	assert_eq!(
		created.tags.as_ref().unwrap().tags,
		vec![("key4".to_string(), "value4".to_string()),].into_iter().collect(),
		"tags changed"
	);
	assert_eq!(created.access_key_id, "access_key_id");
	assert_eq!(created.name, "name");
	assert_eq!(created.region, "us-east-1");
	assert_eq!(created.endpoint, Some("https://endpoint.com".to_string()));
	assert_eq!(created.public_url, Some("https://public_url.com".to_string()));

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_s3_bucket_get() {
	let (global, handler, main_access_token) = scuffle_utilssetup(Default::default()).await;

	let created = vec![
		create_s3_bucket(
			&global,
			main_access_token.organization_id,
			vec![("common".to_string(), "shared".to_string())].into_iter().collect(),
		)
		.await,
		create_s3_bucket(
			&global,
			main_access_token.organization_id,
			vec![("common".to_string(), "shared".to_string())].into_iter().collect(),
		)
		.await,
		create_s3_bucket(
			&global,
			main_access_token.organization_id,
			vec![("common".to_string(), "shared".to_string())].into_iter().collect(),
		)
		.await,
	];

	// Fetch the created tokens using S3BucketGetRequest
	let response: S3BucketGetResponse = process_request(
		&global,
		&main_access_token,
		S3BucketGetRequest {
			ids: created.iter().map(|token| token.id.into()).collect(),
			search_options: None,
		},
	)
	.await
	.unwrap();
	let fetched = response.s3_buckets;

	// Assertions
	assert_eq!(fetched.len(), created.len(), "Should fetch all created s3 bucket");
	for token in fetched {
		let og_key = created
			.iter()
			.find(|&t| t.id == token.id.into_ulid())
			.expect("Fetched keypair must match one of the created ones");
		assert_eq!(token.tags.unwrap().tags, og_key.tags, "Tags should match");
	}

	// Fetch tokens with a specific tag
	let response: S3BucketGetResponse = process_request(
		&global,
		&main_access_token,
		S3BucketGetRequest {
			ids: vec![],
			search_options: Some(SearchOptions {
				limit: 2,
				reverse: false,
				after_id: None,
				tags: Some(Tags {
					tags: vec![("common".to_string(), "shared".to_string())].into_iter().collect(),
				}),
			}),
		},
	)
	.await
	.unwrap();
	let fetched = response.s3_buckets;

	// Assertions for tag-based search
	assert!(!fetched.is_empty(), "Should fetch s3 bucket with specific tags");
	for token in &fetched {
		assert!(
			token.tags.as_ref().unwrap().tags.contains_key("common"),
			"Fetched should contain the 'common' tag"
		);
	}

	// Fetch tokens with limit and reverse options
	let response: S3BucketGetResponse = process_request(
		&global,
		&main_access_token,
		S3BucketGetRequest {
			ids: vec![],
			search_options: Some(SearchOptions {
				limit: 1,
				reverse: true,
				after_id: None,
				tags: None,
			}),
		},
	)
	.await
	.unwrap();
	let fetched = response.s3_buckets;

	// Assertions for limit and reverse options
	assert_eq!(fetched.len(), 1, "Should fetch only one s3 bucket due to limit");

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_s3_bucket_delete() {
	let (global, handler, main_access_token) = scuffle_utilssetup(Default::default()).await;

	let s3_bucket = create_s3_bucket(&global, main_access_token.organization_id, HashMap::new()).await;

	let response: S3BucketDeleteResponse = process_request(
		&global,
		&main_access_token,
		S3BucketDeleteRequest {
			ids: vec![s3_bucket.id.into()],
		},
	)
	.await
	.unwrap();
	let deleted = response.ids;
	let failed_deletions = response.failed_deletes;

	// Assertions for successful deletion
	assert_eq!(deleted.len(), 1, "Should successfully delete one s3 bucket");
	assert!(
		deleted.contains(&s3_bucket.id.into()),
		"Deleted token list should contain the token ID"
	);
	assert!(failed_deletions.is_empty(), "No deletions should fail in this scenario");

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_s3_bucket_boilerplate() {
	let (global, handler, main_access_token) = scuffle_utilssetup(Default::default()).await;

	let no_scopes_token =
		scuffle_utilscreate_access_token(&global, &main_access_token.organization_id, vec![], HashMap::new()).await;

	let server = S3BucketServer::<GlobalState>::new();

	use pb::scuffle::video::v1::s3_bucket_server::S3Bucket as _;

	fn build_request<T>(global: &Arc<GlobalState>, token: &AccessToken, req: T) -> tonic::Request<T> {
		let mut req = tonic::Request::new(req);

		req.extensions_mut().insert(token.clone());
		req.extensions_mut().insert(global.clone());

		req
	}

	let s3_bucket = create_s3_bucket(&global, main_access_token.organization_id, HashMap::new()).await;

	let response = server
		.get(build_request(
			&global,
			&main_access_token,
			S3BucketGetRequest {
				ids: vec![s3_bucket.id.into()],
				search_options: None,
			},
		))
		.await
		.unwrap();
	assert_eq!(response.get_ref().s3_buckets.len(), 1);
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
		.get(build_request(&global, &no_scopes_token, S3BucketGetRequest::default()))
		.await
		.unwrap_err();
	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: s3_bucket:read");

	let response = server
		.create(build_request(
			&global,
			&main_access_token,
			S3BucketCreateRequest {
				access_key_id: "access_key_id".to_string(),
				name: "name".to_string(),
				region: "us-east-1".to_string(),
				secret_access_key: "secret_access_key".to_string(),
				endpoint: None,
				public_url: None,
				tags: None,
			},
		))
		.await
		.unwrap();
	assert!(response.get_ref().s3_bucket.is_some());
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
		.create(build_request(&global, &no_scopes_token, S3BucketCreateRequest::default()))
		.await
		.unwrap_err();
	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: s3_bucket:create");

	let response = server
		.tag(build_request(
			&global,
			&main_access_token,
			S3BucketTagRequest {
				id: Some(s3_bucket.id.into()),
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
		.tag(build_request(&global, &no_scopes_token, S3BucketTagRequest::default()))
		.await
		.unwrap_err();
	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: s3_bucket:modify");

	let response = server
		.untag(build_request(
			&global,
			&main_access_token,
			S3BucketUntagRequest {
				id: Some(s3_bucket.id.into()),
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
		.untag(build_request(&global, &no_scopes_token, S3BucketUntagRequest::default()))
		.await
		.unwrap_err();
	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: s3_bucket:modify");

	let response = server
		.modify(build_request(
			&global,
			&main_access_token,
			S3BucketModifyRequest {
				id: Some(s3_bucket.id.into()),
				tags: Some(Tags {
					tags: vec![("key".to_string(), "value".to_string())].into_iter().collect(),
				}),
				..Default::default()
			},
		))
		.await
		.unwrap();

	assert!(response.get_ref().s3_bucket.is_some());
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
		.modify(build_request(&global, &no_scopes_token, S3BucketModifyRequest::default()))
		.await
		.unwrap_err();

	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: s3_bucket:modify");

	let response = server
		.delete(build_request(
			&global,
			&main_access_token,
			S3BucketDeleteRequest {
				ids: vec![s3_bucket.id.into()],
			},
		))
		.await
		.unwrap();

	assert_eq!(response.get_ref().ids.len(), 1);
	assert_eq!(response.get_ref().failed_deletes.len(), 0); // Cannot delete own token
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
		.delete(build_request(&global, &no_scopes_token, S3BucketDeleteRequest::default()))
		.await
		.unwrap_err();
	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: s3_bucket:delete");

	scuffle_utilsteardown(global, handler).await;
}
