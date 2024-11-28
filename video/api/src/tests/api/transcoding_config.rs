use std::collections::HashMap;
use std::sync::Arc;

use pb::ext::UlidExt;
use pb::scuffle::video::v1::transcoding_config_modify_request::RenditionList;
use pb::scuffle::video::v1::types::{SearchOptions, Tags};
use pb::scuffle::video::v1::{
	TranscodingConfigCreateRequest, TranscodingConfigCreateResponse, TranscodingConfigDeleteRequest,
	TranscodingConfigDeleteResponse, TranscodingConfigGetRequest, TranscodingConfigGetResponse,
	TranscodingConfigModifyRequest, TranscodingConfigModifyResponse, TranscodingConfigTagRequest,
	TranscodingConfigTagResponse, TranscodingConfigUntagRequest, TranscodingConfigUntagResponse,
};
use video_common::database::AccessToken;

use crate::api::transcoding_config::{self, TranscodingConfigServer};
use crate::tests::api::utils::{assert_query_matches, create_transcoding_config, process_request};
use crate::tests::global::GlobalState;
use crate::tests::utils;

#[tokio::test]
async fn test_transcoding_config_get_qb() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let test_cases = vec![
		(
			TranscodingConfigGetRequest {
				ids: vec![access_token.organization_id.into()],
				search_options: None,
			},
			Ok("SELECT * FROM transcoding_configs WHERE organization_id = $1 AND id = ANY($2) ORDER BY id ASC LIMIT 100"),
		),
		(
			TranscodingConfigGetRequest {
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
			Ok(
				"SELECT * FROM transcoding_configs WHERE organization_id = $1 AND id < $2 AND tags @> $3 ORDER BY id DESC LIMIT $4",
			),
		),
	];

	for (req, expected) in test_cases {
		let result = transcoding_config::get::build_query(&req, &access_token);
		assert_query_matches(result, expected);
	}

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_transcoding_config_create_qb() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let test_cases = vec![(
		TranscodingConfigCreateRequest {
			tags: None,
			renditions: vec![
				pb::scuffle::video::v1::types::Rendition::VideoSource as i32,
				pb::scuffle::video::v1::types::Rendition::AudioSource as i32,
			],
		},
		Ok("INSERT INTO transcoding_configs (id,organization_id,renditions,tags) VALUES ($1,$2,$3,$4) RETURNING *"),
	)];

	for (req, expected) in test_cases {
		assert!(transcoding_config::create::validate(&req).is_ok());
		let result = transcoding_config::create::build_query(&req, &access_token);
		assert_query_matches(result, expected);
	}

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_transcoding_config_modify_qb() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let test_cases = vec![
		(
			TranscodingConfigModifyRequest {
				id: Some(access_token.id.into()),
				tags: None,
				renditions: Some(RenditionList {
					items: vec![
						pb::scuffle::video::v1::types::Rendition::VideoSource as i32,
						pb::scuffle::video::v1::types::Rendition::AudioSource as i32,
					],
				}),
			},
			Ok(
				"UPDATE transcoding_configs SET renditions = $1,updated_at = NOW() WHERE id = $2 AND organization_id = $3 RETURNING *",
			),
		),
		(
			TranscodingConfigModifyRequest {
				id: Some(access_token.id.into()),
				tags: Some(Tags {
					tags: vec![("example_tag".to_string(), "example_value".to_string())]
						.into_iter()
						.collect(),
				}),
				renditions: None,
			},
			Ok(
				"UPDATE transcoding_configs SET tags = $1,updated_at = NOW() WHERE id = $2 AND organization_id = $3 RETURNING *",
			),
		),
		(
			TranscodingConfigModifyRequest {
				id: Some(access_token.id.into()),
				tags: Some(Tags {
					tags: vec![("example_tag".to_string(), "example_value".to_string())]
						.into_iter()
						.collect(),
				}),
				renditions: Some(RenditionList {
					items: vec![
						pb::scuffle::video::v1::types::Rendition::VideoSource as i32,
						pb::scuffle::video::v1::types::Rendition::AudioSource as i32,
					],
				}),
			},
			Ok(
				"UPDATE transcoding_configs SET renditions = $1,tags = $2,updated_at = NOW() WHERE id = $3 AND organization_id = $4 RETURNING *",
			),
		),
		(
			TranscodingConfigModifyRequest {
				id: Some(access_token.id.into()),
				tags: None,
				renditions: None,
			},
			Err("at least one field must be set to modify"),
		),
	];

	for (req, expected) in test_cases {
		assert!(transcoding_config::modify::validate(&req).is_ok());
		let result = transcoding_config::modify::build_query(&req, &access_token);
		assert_query_matches(result, expected);
	}

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_transcoding_config_tag_qb() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let test_cases = vec![(
		TranscodingConfigTagRequest {
			id: Some(access_token.id.into()),
			tags: Some(Tags {
				tags: vec![("example_tag".to_string(), "example_value".to_string())]
					.into_iter()
					.collect(),
			}),
		},
		Ok(
			"WITH mt AS (SELECT id, tags || $1 AS new_tags, CASE WHEN tags @> $1 THEN 1 WHEN COUNT(jsonb_object_keys(tags || $1)) > $2 THEN 2 ELSE 0 END AS status FROM transcoding_configs WHERE id = $3 AND organization_id = $4 GROUP BY id, organization_id) UPDATE transcoding_configs AS t SET tags = CASE WHEN mt.status = 0 THEN mt.new_tags ELSE tags END, updated_at = CASE WHEN mt.status = 0 THEN now() ELSE updated_at END FROM mt WHERE t.id = mt.id RETURNING t.tags as tags, mt.status as status;",
		),
	)];

	for (req, expected) in test_cases {
		assert!(transcoding_config::tag::validate(&req).is_ok());
		let result = transcoding_config::tag::build_query(&req, &access_token);
		assert_query_matches(result, expected);
	}

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_transcoding_config_untag_qb() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let test_cases = vec![(
		TranscodingConfigUntagRequest {
			id: Some(access_token.id.into()),
			tags: vec!["example_tag".to_string()],
		},
		Ok(
			"WITH rt AS (SELECT id, tags - $1::TEXT[] AS new_tags, CASE WHEN NOT tags ?| $1 THEN 1 ELSE 0 END AS status FROM transcoding_configs WHERE id = $2 AND organization_id = $3 GROUP BY id, organization_id) UPDATE transcoding_configs AS t SET tags = CASE WHEN rt.status = 0 THEN rt.new_tags ELSE tags END, updated_at = CASE WHEN rt.status = 0 THEN now() ELSE updated_at END FROM rt WHERE t.id = rt.id RETURNING t.tags AS tags, rt.status AS status;",
		),
	)];

	for (req, expected) in test_cases {
		assert!(transcoding_config::untag::validate(&req).is_ok());
		let result = transcoding_config::untag::build_query(&req, &access_token);
		assert_query_matches(result, expected);
	}

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_transcoding_config_tag() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let transcoding_config = create_transcoding_config(
		&global,
		access_token.organization_id,
		vec![("key".to_string(), "value".to_string())].into_iter().collect(),
	)
	.await;

	let response: TranscodingConfigTagResponse = process_request(
		&global,
		&access_token,
		TranscodingConfigTagRequest {
			id: Some(transcoding_config.id.into()),
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
async fn test_transcoding_config_untag() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let transcoding_config = create_transcoding_config(
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

	let response: TranscodingConfigUntagResponse = process_request(
		&global,
		&access_token,
		TranscodingConfigUntagRequest {
			id: Some(transcoding_config.id.into()),
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
async fn test_transcoding_config_create() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let response: TranscodingConfigCreateResponse = process_request(
		&global,
		&access_token,
		TranscodingConfigCreateRequest {
			renditions: vec![
				pb::scuffle::video::v1::types::Rendition::VideoSource as i32,
				pb::scuffle::video::v1::types::Rendition::AudioSource as i32,
			],
			tags: None,
		},
	)
	.await
	.unwrap();
	let created = response.transcoding_config.as_ref().unwrap();
	assert!(created.tags.is_none() || created.tags.as_ref().unwrap().tags.is_empty());

	let response: TranscodingConfigCreateResponse = process_request(
		&global,
		&access_token,
		TranscodingConfigCreateRequest {
			renditions: vec![
				pb::scuffle::video::v1::types::Rendition::VideoSource as i32,
				pb::scuffle::video::v1::types::Rendition::AudioSource as i32,
			],
			tags: Some(Tags {
				tags: vec![("tag_key".to_string(), "tag_value".to_string())].into_iter().collect(),
			}),
		},
	)
	.await
	.unwrap();
	let created = response.transcoding_config.as_ref().unwrap();
	assert_eq!(created.tags.as_ref().unwrap().tags.get("tag_key").unwrap(), "tag_value");
	assert_eq!(created.tags.as_ref().unwrap().tags.len(), 1, "1 tag");
	assert_eq!(created.renditions.len(), 2, "1 lifecycle policy");
	assert_eq!(
		created.renditions,
		vec![
			pb::scuffle::video::v1::types::Rendition::VideoSource as i32,
			pb::scuffle::video::v1::types::Rendition::AudioSource as i32,
		]
	);

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_transcoding_config_modify() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let transcoding_config = create_transcoding_config(
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

	let response: TranscodingConfigModifyResponse = process_request(
		&global,
		&access_token,
		TranscodingConfigModifyRequest {
			id: Some(transcoding_config.id.into()),
			renditions: None,
			tags: Some(Tags {
				tags: vec![("key3".to_string(), "value3".to_string())].into_iter().collect(),
			}),
		},
	)
	.await
	.unwrap();
	let created = response.transcoding_config.as_ref().unwrap();

	assert_eq!(
		created.tags.as_ref().unwrap().tags,
		vec![("key3".to_string(), "value3".to_string()),].into_iter().collect(),
		"tags changed"
	);

	let response: TranscodingConfigModifyResponse = process_request(
		&global,
		&access_token,
		TranscodingConfigModifyRequest {
			id: Some(transcoding_config.id.into()),
			renditions: Some(RenditionList {
				items: vec![
					pb::scuffle::video::v1::types::Rendition::VideoSd as i32,
					pb::scuffle::video::v1::types::Rendition::AudioSource as i32,
				],
			}),
			tags: None,
		},
	)
	.await
	.unwrap();
	let created = response.transcoding_config.as_ref().unwrap();

	assert_eq!(created.renditions.len(), 2, "1 lifecycle policy");
	assert_eq!(
		created.renditions,
		vec![
			pb::scuffle::video::v1::types::Rendition::VideoSd as i32,
			pb::scuffle::video::v1::types::Rendition::AudioSource as i32,
		]
	);

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_transcoding_config_get() {
	let (global, handler, main_access_token) = scuffle_utilssetup(Default::default()).await;

	let created = vec![
		create_transcoding_config(
			&global,
			main_access_token.organization_id,
			vec![("common".to_string(), "shared".to_string())].into_iter().collect(),
		)
		.await,
		create_transcoding_config(
			&global,
			main_access_token.organization_id,
			vec![("common1".to_string(), "shared1".to_string())].into_iter().collect(),
		)
		.await,
		create_transcoding_config(
			&global,
			main_access_token.organization_id,
			vec![("common2".to_string(), "shared2".to_string())].into_iter().collect(),
		)
		.await,
	];

	// Fetch the created tokens using TranscodingConfigGetRequest
	let response: TranscodingConfigGetResponse = process_request(
		&global,
		&main_access_token,
		TranscodingConfigGetRequest {
			ids: created.iter().map(|token| token.id.into()).collect(),
			search_options: None,
		},
	)
	.await
	.unwrap();
	let fetched = response.transcoding_configs;

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
	let response: TranscodingConfigGetResponse = process_request(
		&global,
		&main_access_token,
		TranscodingConfigGetRequest {
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
	let fetched = response.transcoding_configs;

	// Assertions for tag-based search
	assert!(!fetched.is_empty(), "Should fetch playback key pair with specific tags");
	for token in &fetched {
		assert!(
			token.tags.as_ref().unwrap().tags.contains_key("common"),
			"Fetched should contain the 'common' tag"
		);
	}

	// Fetch tokens with limit and reverse options
	let response: TranscodingConfigGetResponse = process_request(
		&global,
		&main_access_token,
		TranscodingConfigGetRequest {
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
	let fetched = response.transcoding_configs;

	// Assertions for limit and reverse options
	assert_eq!(fetched.len(), 1, "Should fetch only one playback key pair due to limit");

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_transcoding_config_delete() {
	let (global, handler, main_access_token) = scuffle_utilssetup(Default::default()).await;

	let transcoding_config = create_transcoding_config(
		&global,
		main_access_token.organization_id,
		vec![("key".to_string(), "value".to_string())].into_iter().collect(),
	)
	.await;

	let response: TranscodingConfigDeleteResponse = process_request(
		&global,
		&main_access_token,
		TranscodingConfigDeleteRequest {
			ids: vec![transcoding_config.id.into()],
		},
	)
	.await
	.unwrap();
	let deleted = response.ids;
	let failed_deletions = response.failed_deletes;

	// Assertions for successful deletion
	assert_eq!(deleted.len(), 1, "Should successfully delete one playback key pair");
	assert!(
		deleted.contains(&transcoding_config.id.into()),
		"Deleted token list should contain the token ID"
	);
	assert!(failed_deletions.is_empty(), "No deletions should fail in this scenario");

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_transcoding_config_boiler_plate() {
	let (global, handler, main_access_token) = scuffle_utilssetup(Default::default()).await;

	let no_scopes_token =
		scuffle_utilscreate_access_token(&global, &main_access_token.organization_id, vec![], HashMap::new()).await;

	let server = TranscodingConfigServer::<GlobalState>::new();

	use pb::scuffle::video::v1::transcoding_config_server::TranscodingConfig as _;

	fn build_request<T>(global: &Arc<GlobalState>, token: &AccessToken, req: T) -> tonic::Request<T> {
		let mut req = tonic::Request::new(req);

		req.extensions_mut().insert(token.clone());
		req.extensions_mut().insert(global.clone());

		req
	}

	let transcoding_config = create_transcoding_config(
		&global,
		main_access_token.organization_id,
		vec![("key".to_string(), "value".to_string())].into_iter().collect(),
	)
	.await;

	let response = server
		.get(build_request(
			&global,
			&main_access_token,
			TranscodingConfigGetRequest {
				ids: vec![transcoding_config.id.into()],
				search_options: None,
			},
		))
		.await
		.unwrap();
	assert_eq!(response.get_ref().transcoding_configs.len(), 1);
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
		.get(build_request(
			&global,
			&no_scopes_token,
			TranscodingConfigGetRequest::default(),
		))
		.await
		.unwrap_err();
	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: transcoding_config:read");

	let response = server
		.create(build_request(
			&global,
			&main_access_token,
			TranscodingConfigCreateRequest {
				renditions: vec![
					pb::scuffle::video::v1::types::Rendition::VideoSource as i32,
					pb::scuffle::video::v1::types::Rendition::AudioSource as i32,
				],
				tags: None,
			},
		))
		.await
		.unwrap();
	assert!(response.get_ref().transcoding_config.is_some());
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
			TranscodingConfigCreateRequest::default(),
		))
		.await
		.unwrap_err();
	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: transcoding_config:create");

	let response = server
		.tag(build_request(
			&global,
			&main_access_token,
			TranscodingConfigTagRequest {
				id: Some(transcoding_config.id.into()),
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
		.tag(build_request(
			&global,
			&no_scopes_token,
			TranscodingConfigTagRequest::default(),
		))
		.await
		.unwrap_err();
	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: transcoding_config:modify");

	let response = server
		.untag(build_request(
			&global,
			&main_access_token,
			TranscodingConfigUntagRequest {
				id: Some(transcoding_config.id.into()),
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
			TranscodingConfigUntagRequest::default(),
		))
		.await
		.unwrap_err();
	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: transcoding_config:modify");

	let response = server
		.modify(build_request(
			&global,
			&main_access_token,
			TranscodingConfigModifyRequest {
				id: Some(transcoding_config.id.into()),
				tags: Some(Tags {
					tags: vec![("key".to_string(), "value".to_string())].into_iter().collect(),
				}),
				renditions: None,
			},
		))
		.await
		.unwrap();

	assert!(response.get_ref().transcoding_config.is_some());
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
			TranscodingConfigModifyRequest::default(),
		))
		.await
		.unwrap_err();

	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: transcoding_config:modify");

	let response = server
		.delete(build_request(
			&global,
			&main_access_token,
			TranscodingConfigDeleteRequest {
				ids: vec![transcoding_config.id.into()],
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
			TranscodingConfigDeleteRequest::default(),
		))
		.await
		.unwrap_err();
	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: transcoding_config:delete");

	scuffle_utilsteardown(global, handler).await;
}
