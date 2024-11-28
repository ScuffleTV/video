use std::collections::HashMap;
use std::sync::Arc;

use pb::ext::UlidExt;
use pb::scuffle::video::v1::types::{SearchOptions, Tags};
use pb::scuffle::video::v1::{
	PlaybackKeyPairCreateRequest, PlaybackKeyPairCreateResponse, PlaybackKeyPairDeleteRequest,
	PlaybackKeyPairDeleteResponse, PlaybackKeyPairGetRequest, PlaybackKeyPairGetResponse, PlaybackKeyPairModifyRequest,
	PlaybackKeyPairModifyResponse, PlaybackKeyPairTagRequest, PlaybackKeyPairTagResponse, PlaybackKeyPairUntagRequest,
	PlaybackKeyPairUntagResponse,
};
use video_common::database::AccessToken;

use crate::api::playback_key_pair::utils::validate_public_key;
use crate::api::playback_key_pair::{self, PlaybackKeyPairServer};
use crate::tests::api::utils::{assert_query_matches, create_playback_keypair, process_request};
use crate::tests::global::GlobalState;
use crate::tests::utils;

#[tokio::test]
async fn test_playback_key_pair_get_qb() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let playback_key_pair = create_playback_keypair(
		&global,
		access_token.organization_id,
		vec![("key".to_string(), "value".to_string())].into_iter().collect(),
	)
	.await;

	let test_cases = vec![
		(
			PlaybackKeyPairGetRequest {
				ids: vec![playback_key_pair.id.into()],
				search_options: None,
			},
			Ok("SELECT * FROM playback_key_pairs WHERE organization_id = $1 AND id = ANY($2) ORDER BY id ASC LIMIT 100"),
		),
		(
			PlaybackKeyPairGetRequest {
				ids: vec![playback_key_pair.id.into()],
				search_options: Some(SearchOptions {
					limit: 1,
					reverse: true,
					after_id: Some(playback_key_pair.id.into()),
					tags: None,
				}),
			},
			Ok(
				"SELECT * FROM playback_key_pairs WHERE organization_id = $1 AND id = ANY($2) AND id < $3 ORDER BY id DESC LIMIT $4",
			),
		),
	];

	for (req, expected) in test_cases {
		let result = playback_key_pair::get::build_query(&req, &access_token);
		assert_query_matches(result, expected);
	}

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_playback_key_pair_create_qb() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let test_cases = vec![(
		PlaybackKeyPairCreateRequest {
			tags: None,
			public_key: include_str!("../certs/ec384/public.pem").to_string(),
		},
		Ok(
			"INSERT INTO playback_key_pairs (id,organization_id,public_key,fingerprint,updated_at,tags) VALUES ($1,$2,$3,$4,$5,$6) RETURNING *",
		),
	)];

	for (req, expected) in test_cases {
		let result =
			playback_key_pair::create::build_query(&req, &access_token, playback_key_pair::create::validate(&req).unwrap());
		assert_query_matches(result, expected);
	}

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_playback_key_pair_modify_qb() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let playback_key_pair = create_playback_keypair(
		&global,
		access_token.organization_id,
		vec![("key".to_string(), "value".to_string())].into_iter().collect(),
	)
	.await;

	let test_cases = vec![
		(
			PlaybackKeyPairModifyRequest {
				id: Some(playback_key_pair.id.into()),
				tags: None,
				public_key: Some(include_str!("../certs/ec384/public.pem").to_string()),
			},
			Ok(
				"UPDATE playback_key_pairs SET public_key = $1,fingerprint = $2,updated_at = $3 WHERE id = $4 AND organization_id = $5 RETURNING *",
			),
		),
		(
			PlaybackKeyPairModifyRequest {
				id: Some(playback_key_pair.id.into()),
				tags: Some(Tags {
					tags: vec![("example_tag".to_string(), "example_value".to_string())]
						.into_iter()
						.collect(),
				}),
				public_key: Some(include_str!("../certs/ec384/public.pem").to_string()),
			},
			Ok(
				"UPDATE playback_key_pairs SET tags = $1,public_key = $2,fingerprint = $3,updated_at = $4 WHERE id = $5 AND organization_id = $6 RETURNING *",
			),
		),
		(
			PlaybackKeyPairModifyRequest {
				id: Some(playback_key_pair.id.into()),
				tags: None,
				public_key: None,
			},
			Err("at least one field must be set to modify"),
		),
	];

	for (req, expected) in test_cases {
		assert!(playback_key_pair::modify::validate(&req).is_ok());
		let result = playback_key_pair::modify::build_query(&req, &access_token);
		assert_query_matches(result, expected);
	}

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_playback_key_pair_tag_qb() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let playback_key_pair = create_playback_keypair(
		&global,
		access_token.organization_id,
		vec![("key".to_string(), "value".to_string())].into_iter().collect(),
	)
	.await;

	let test_cases = vec![(
		PlaybackKeyPairTagRequest {
			id: Some(playback_key_pair.id.into()),
			tags: Some(Tags {
				tags: vec![("example_tag".to_string(), "example_value".to_string())]
					.into_iter()
					.collect(),
			}),
		},
		Ok(
			"WITH mt AS (SELECT id, tags || $1 AS new_tags, CASE WHEN tags @> $1 THEN 1 WHEN COUNT(jsonb_object_keys(tags || $1)) > $2 THEN 2 ELSE 0 END AS status FROM playback_key_pairs WHERE id = $3 AND organization_id = $4 GROUP BY id, organization_id) UPDATE playback_key_pairs AS t SET tags = CASE WHEN mt.status = 0 THEN mt.new_tags ELSE tags END, updated_at = CASE WHEN mt.status = 0 THEN now() ELSE updated_at END FROM mt WHERE t.id = mt.id RETURNING t.tags as tags, mt.status as status;",
		),
	)];

	for (req, expected) in test_cases {
		assert!(playback_key_pair::tag::validate(&req).is_ok());
		let result = playback_key_pair::tag::build_query(&req, &access_token);
		assert_query_matches(result, expected);
	}

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_playback_key_pair_untag_qb() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let playback_key_pair = create_playback_keypair(
		&global,
		access_token.organization_id,
		vec![("key".to_string(), "value".to_string())].into_iter().collect(),
	)
	.await;

	let test_cases = vec![(
		PlaybackKeyPairUntagRequest {
			id: Some(playback_key_pair.id.into()),
			tags: vec!["example_tag".to_string()],
		},
		Ok(
			"WITH rt AS (SELECT id, tags - $1::TEXT[] AS new_tags, CASE WHEN NOT tags ?| $1 THEN 1 ELSE 0 END AS status FROM playback_key_pairs WHERE id = $2 AND organization_id = $3 GROUP BY id, organization_id) UPDATE playback_key_pairs AS t SET tags = CASE WHEN rt.status = 0 THEN rt.new_tags ELSE tags END, updated_at = CASE WHEN rt.status = 0 THEN now() ELSE updated_at END FROM rt WHERE t.id = rt.id RETURNING t.tags AS tags, rt.status AS status;",
		),
	)];

	for (req, expected) in test_cases {
		assert!(playback_key_pair::untag::validate(&req).is_ok());
		let result = playback_key_pair::untag::build_query(&req, &access_token);
		assert_query_matches(result, expected);
	}

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_playback_key_pair_tag() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let playback_key_pair = create_playback_keypair(
		&global,
		access_token.organization_id,
		vec![("key".to_string(), "value".to_string())].into_iter().collect(),
	)
	.await;

	let tag_request = PlaybackKeyPairTagRequest {
		id: Some(playback_key_pair.id.into()),
		tags: Some(Tags {
			tags: vec![("key2".to_string(), "value2".to_string())].into_iter().collect(),
		}),
	};

	let response: PlaybackKeyPairTagResponse = process_request(&global, &access_token, tag_request)
		.await
		.expect("Tagging should be successful");
	let tags = response.tags.unwrap();

	assert_eq!(tags.tags.get("key").unwrap(), &"value");
	assert_eq!(tags.tags.get("key2").unwrap(), &"value2");

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_playback_key_pair_untag() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let playback_key_pair = create_playback_keypair(
		&global,
		access_token.organization_id,
		vec![
			("key".to_string(), "value".to_string()),
			("key2".to_string(), "value2".to_string()),
		]
		.into_iter()
		.collect(),
	)
	.await;

	// Now, untag
	let untag_request = PlaybackKeyPairUntagRequest {
		id: Some(playback_key_pair.id.into()),
		tags: vec!["key".to_string()],
	};

	let response: PlaybackKeyPairUntagResponse = process_request(&global, &access_token, untag_request)
		.await
		.expect("Untagging should be successful");
	let tags = response.tags.unwrap();
	assert_eq!(tags.tags.len(), 1, "Only 1 tag should be left");
	assert_eq!(tags.tags.get("key2").unwrap(), &"value2");

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_playback_key_pair_create() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let (_, fingerprint) = validate_public_key(include_str!("../certs/ec384/public.pem")).unwrap();

	// Test case: Create a basic playback key pair
	let req = PlaybackKeyPairCreateRequest {
		public_key: include_str!("../certs/ec384/public.pem").to_string(),
		tags: None,
	};

	let response: PlaybackKeyPairCreateResponse = process_request(&global, &access_token, req).await.unwrap();
	let created = response.playback_key_pair.as_ref().unwrap();
	assert!(created.tags.is_none() || created.tags.as_ref().unwrap().tags.is_empty());

	assert_eq!(created.fingerprint, fingerprint, "Fingerprint should match the public key");

	// Test case: Create an playback key pair with specific tags
	let req = PlaybackKeyPairCreateRequest {
		public_key: include_str!("../certs/ec384/public.pem").to_string(),
		tags: Some(Tags {
			tags: vec![("tag_key".to_string(), "tag_value".to_string())].into_iter().collect(),
		}),
	};

	let response: PlaybackKeyPairCreateResponse = process_request(&global, &access_token, req).await.unwrap();
	let created = response.playback_key_pair.as_ref().unwrap();
	assert_eq!(created.tags.as_ref().unwrap().tags.get("tag_key").unwrap(), "tag_value");

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_playback_key_pair_modify() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let playback_key_pair = create_playback_keypair(
		&global,
		access_token.organization_id,
		vec![
			("key".to_string(), "value".to_string()),
			("key2".to_string(), "value2".to_string()),
		]
		.into_iter()
		.collect(),
	)
	.await;

	// Test case: Create a basic playback key pair
	let req = PlaybackKeyPairModifyRequest {
		id: Some(playback_key_pair.id.into()),
		public_key: Some(include_str!("../certs/ec384/public.pem").to_string()),
		tags: None,
	};

	let response: PlaybackKeyPairModifyResponse = process_request(&global, &access_token, req).await.unwrap();
	let created = response.playback_key_pair.as_ref().unwrap();

	assert_eq!(
		created.fingerprint, playback_key_pair.fingerprint,
		"Fingerprint should match the public key"
	);

	assert_eq!(
		created.tags.as_ref().unwrap().tags,
		vec![
			("key".to_string(), "value".to_string()),
			("key2".to_string(), "value2".to_string()),
		]
		.into_iter()
		.collect(),
		"tags should not change"
	);

	// Test case: Create an playback key pair with specific tags
	let req = PlaybackKeyPairModifyRequest {
		id: Some(playback_key_pair.id.into()),
		public_key: None,
		tags: Some(Tags {
			tags: vec![("tag_key".to_string(), "tag_value".to_string())].into_iter().collect(),
		}),
	};

	let response: PlaybackKeyPairModifyResponse = process_request(&global, &access_token, req).await.unwrap();
	let created = response.playback_key_pair.as_ref().unwrap();
	assert_eq!(created.tags.as_ref().unwrap().tags.get("tag_key").unwrap(), "tag_value");
	assert_eq!(created.tags.as_ref().unwrap().tags.len(), 1, "Only one tag should be left");

	assert_eq!(
		created.fingerprint, playback_key_pair.fingerprint,
		"Fingerprint should not change"
	);

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_playback_key_pair_get() {
	let (global, handler, main_access_token) = scuffle_utilssetup(Default::default()).await;

	// Create multiple playback key pair with different tags for testing
	let created = vec![
		create_playback_keypair(
			&global,
			main_access_token.organization_id,
			vec![("common".to_string(), "shared".to_string())].into_iter().collect(),
		)
		.await,
		create_playback_keypair(
			&global,
			main_access_token.organization_id,
			vec![("common1".to_string(), "shared1".to_string())].into_iter().collect(),
		)
		.await,
		create_playback_keypair(
			&global,
			main_access_token.organization_id,
			vec![("common2".to_string(), "shared2".to_string())].into_iter().collect(),
		)
		.await,
	];

	// Fetch the created tokens using PlaybackKeyPairGetRequest
	let req = PlaybackKeyPairGetRequest {
		ids: created.iter().map(|token| token.id.into()).collect(),
		search_options: None,
	};

	let response: PlaybackKeyPairGetResponse = process_request(&global, &main_access_token, req).await.unwrap();
	let fetched = response.playback_key_pairs;

	// Assertions
	assert_eq!(fetched.len(), created.len(), "Should fetch all created playback key pair");
	for token in fetched {
		let og_key = created
			.iter()
			.find(|&t| t.id == token.id.into_ulid())
			.expect("Fetched keypair must match one of the created ones");
		assert_eq!(token.tags.unwrap().tags, og_key.tags, "Tags should match");
	}

	// Fetch tokens with a specific tag
	let req = PlaybackKeyPairGetRequest {
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

	let response: PlaybackKeyPairGetResponse = process_request(&global, &main_access_token, req).await.unwrap();
	let fetched = response.playback_key_pairs;

	// Assertions for tag-based search
	assert!(!fetched.is_empty(), "Should fetch playback key pair with specific tags");
	for token in &fetched {
		assert!(
			token.tags.as_ref().unwrap().tags.contains_key("common"),
			"Fetched should contain the 'common' tag"
		);
	}

	// Fetch tokens with limit and reverse options
	let req = PlaybackKeyPairGetRequest {
		ids: vec![],
		search_options: Some(SearchOptions {
			limit: 1,
			reverse: true,
			after_id: None,
			tags: None,
		}),
	};

	let response: PlaybackKeyPairGetResponse = process_request(&global, &main_access_token, req).await.unwrap();
	let fetched = response.playback_key_pairs;

	// Assertions for limit and reverse options
	assert_eq!(fetched.len(), 1, "Should fetch only one playback key pair due to limit");

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_playback_key_pair_delete() {
	let (global, handler, main_access_token) = scuffle_utilssetup(Default::default()).await;

	// Create access tokens to be deleted
	let keypair_to_delete = create_playback_keypair(
		&global,
		main_access_token.organization_id,
		vec![("key".to_string(), "value".to_string())].into_iter().collect(),
	)
	.await;

	// Delete request with a token the caller should have permission to delete
	let req = PlaybackKeyPairDeleteRequest {
		ids: vec![keypair_to_delete.id.into()],
	};

	let response: PlaybackKeyPairDeleteResponse = process_request(&global, &main_access_token, req).await.unwrap();
	let deleted = response.ids;
	let failed_deletions = response.failed_deletes;

	// Assertions for successful deletion
	assert_eq!(deleted.len(), 1, "Should successfully delete one playback key pair");
	assert!(
		deleted.contains(&keypair_to_delete.id.into()),
		"Deleted token list should contain the token ID"
	);
	assert!(failed_deletions.is_empty(), "No deletions should fail in this scenario");

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_playback_key_pair_boiler_plate() {
	let (global, handler, main_access_token) = scuffle_utilssetup(Default::default()).await;

	let no_scopes_token =
		scuffle_utilscreate_access_token(&global, &main_access_token.organization_id, vec![], HashMap::new()).await;

	let server = PlaybackKeyPairServer::<GlobalState>::new();

	use pb::scuffle::video::v1::playback_key_pair_server::PlaybackKeyPair as _;

	fn build_request<T>(global: &Arc<GlobalState>, token: &AccessToken, req: T) -> tonic::Request<T> {
		let mut req = tonic::Request::new(req);

		req.extensions_mut().insert(token.clone());
		req.extensions_mut().insert(global.clone());

		req
	}

	let keypair = create_playback_keypair(
		&global,
		main_access_token.organization_id,
		vec![("key".to_string(), "value".to_string())].into_iter().collect(),
	)
	.await;

	let response = server
		.get(build_request(
			&global,
			&main_access_token,
			PlaybackKeyPairGetRequest {
				ids: vec![keypair.id.into()],
				search_options: None,
			},
		))
		.await
		.unwrap();
	assert_eq!(response.get_ref().playback_key_pairs.len(), 1);
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
		.get(build_request(&global, &no_scopes_token, PlaybackKeyPairGetRequest::default()))
		.await
		.unwrap_err();
	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: playback_key_pair:read");

	let response = server
		.create(build_request(
			&global,
			&main_access_token,
			PlaybackKeyPairCreateRequest {
				public_key: include_str!("../certs/ec384/public.pem").to_string(),
				tags: None,
			},
		))
		.await
		.unwrap();
	assert!(response.get_ref().playback_key_pair.is_some());
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
		.create(build_request(
			&global,
			&no_scopes_token,
			PlaybackKeyPairCreateRequest::default(),
		))
		.await
		.unwrap_err();
	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: playback_key_pair:create");

	let response = server
		.tag(build_request(
			&global,
			&main_access_token,
			PlaybackKeyPairTagRequest {
				id: Some(keypair.id.into()),
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
		.tag(build_request(&global, &no_scopes_token, PlaybackKeyPairTagRequest::default()))
		.await
		.unwrap_err();
	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: playback_key_pair:modify");

	let response = server
		.untag(build_request(
			&global,
			&main_access_token,
			PlaybackKeyPairUntagRequest {
				id: Some(keypair.id.into()),
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
		.untag(build_request(
			&global,
			&no_scopes_token,
			PlaybackKeyPairUntagRequest::default(),
		))
		.await
		.unwrap_err();
	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: playback_key_pair:modify");

	let response = server
		.modify(build_request(
			&global,
			&main_access_token,
			PlaybackKeyPairModifyRequest {
				id: Some(keypair.id.into()),
				tags: None,
				public_key: Some(include_str!("../certs/ec384/public.pem").to_string()),
			},
		))
		.await
		.unwrap();

	assert!(response.get_ref().playback_key_pair.is_some());
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
		.modify(build_request(
			&global,
			&no_scopes_token,
			PlaybackKeyPairModifyRequest::default(),
		))
		.await
		.unwrap_err();

	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: playback_key_pair:modify");

	let response = server
		.delete(build_request(
			&global,
			&main_access_token,
			PlaybackKeyPairDeleteRequest {
				ids: vec![keypair.id.into()],
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
		.delete(build_request(
			&global,
			&no_scopes_token,
			PlaybackKeyPairDeleteRequest::default(),
		))
		.await
		.unwrap_err();
	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: playback_key_pair:delete");

	scuffle_utilsteardown(global, handler).await;
}
