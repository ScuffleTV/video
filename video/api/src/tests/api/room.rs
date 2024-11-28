use std::collections::HashMap;
use std::sync::Arc;

use ::utils::prelude::FutureTimeout;
use binary_helper::global::{GlobalDb, GlobalNats};
use futures_util::StreamExt;
use pb::scuffle::video::v1::types::{SearchOptions, Tags};
use pb::scuffle::video::v1::{
	RoomCreateRequest, RoomCreateResponse, RoomDeleteRequest, RoomDeleteResponse, RoomDisconnectRequest,
	RoomDisconnectResponse, RoomGetRequest, RoomGetResponse, RoomModifyRequest, RoomModifyResponse, RoomResetKeyRequest,
	RoomResetKeyResponse, RoomTagRequest, RoomTagResponse, RoomUntagRequest, RoomUntagResponse,
};
use ulid::Ulid;
use video_common::database::{AccessToken, RoomStatus, Visibility};

use crate::api::room::{self, RoomServer};
use crate::tests::api::utils::{
	assert_query_matches, create_recording_config, create_room, create_s3_bucket, create_transcoding_config, process_request,
};
use crate::tests::global::GlobalState;
use crate::tests::utils;

#[tokio::test]
async fn test_room_get_qb() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let test_cases = vec![
		(
			RoomGetRequest {
				ids: vec![access_token.id.into()],
				transcoding_config_id: None,
				recording_config_id: None,
				visibility: None,
				status: None,
				search_options: None,
			},
			Ok("SELECT * FROM rooms WHERE organization_id = $1 AND id = ANY($2) ORDER BY id ASC LIMIT 100"),
		),
		(
			RoomGetRequest {
				ids: vec![access_token.id.into()],
				transcoding_config_id: None,
				recording_config_id: None,
				visibility: None,
				status: None,
				search_options: Some(SearchOptions {
					limit: 1,
					reverse: true,
					after_id: Some(access_token.id.into()),
					tags: None,
				}),
			},
			Ok("SELECT * FROM rooms WHERE organization_id = $1 AND id = ANY($2) AND id < $3 ORDER BY id DESC LIMIT $4"),
		),
		(
			RoomGetRequest {
				ids: vec![],
				transcoding_config_id: Some(access_token.id.into()),
				recording_config_id: Some(access_token.id.into()),
				visibility: None,
				status: None,
				search_options: None,
			},
			Ok(
				"SELECT * FROM rooms WHERE organization_id = $1 AND transcoding_config_id = $2 AND recording_config_id = $3 ORDER BY id ASC LIMIT 100",
			),
		),
		(
			RoomGetRequest {
				ids: vec![],
				transcoding_config_id: None,
				recording_config_id: None,
				visibility: Some(pb::scuffle::video::v1::types::Visibility::Public as i32),
				status: None,
				search_options: None,
			},
			Ok("SELECT * FROM rooms WHERE organization_id = $1 AND visibility = $2 ORDER BY id ASC LIMIT 100"),
		),
		(
			RoomGetRequest {
				ids: vec![],
				transcoding_config_id: None,
				recording_config_id: None,
				visibility: Some(pb::scuffle::video::v1::types::Visibility::Private as i32),
				status: None,
				search_options: None,
			},
			Ok("SELECT * FROM rooms WHERE organization_id = $1 AND visibility = $2 ORDER BY id ASC LIMIT 100"),
		),
		(
			RoomGetRequest {
				ids: vec![],
				transcoding_config_id: None,
				recording_config_id: None,
				visibility: None,
				status: Some(pb::scuffle::video::v1::types::RoomStatus::Ready as i32),
				search_options: None,
			},
			Ok("SELECT * FROM rooms WHERE organization_id = $1 AND status = $2 ORDER BY id ASC LIMIT 100"),
		),
		(
			RoomGetRequest {
				ids: vec![],
				transcoding_config_id: None,
				recording_config_id: None,
				visibility: None,
				status: Some(pb::scuffle::video::v1::types::RoomStatus::Offline as i32),
				search_options: None,
			},
			Ok("SELECT * FROM rooms WHERE organization_id = $1 AND status = $2 ORDER BY id ASC LIMIT 100"),
		),
	];

	for (req, expected) in test_cases {
		let result = room::get::build_query(&req, &access_token);
		assert_query_matches(result, expected);
	}

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_room_create_qb() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let s3_bucket = create_s3_bucket(&global, access_token.organization_id, HashMap::new()).await;
	let recording_config =
		create_recording_config(&global, access_token.organization_id, s3_bucket.id, HashMap::new()).await;
	let transcoding_config = create_transcoding_config(&global, access_token.organization_id, HashMap::new()).await;

	let test_cases = vec![
		(
			RoomCreateRequest {
				transcoding_config_id: None,
				recording_config_id: None,
				visibility: pb::scuffle::video::v1::types::Visibility::Public as i32,
				tags: None,
			},
			Ok(
				"INSERT INTO rooms (id,organization_id,transcoding_config_id,recording_config_id,visibility,stream_key,tags) VALUES ($1,$2,$3,$4,$5,$6,$7) RETURNING *",
			),
		),
		(
			RoomCreateRequest {
				transcoding_config_id: Some(transcoding_config.id.into()),
				recording_config_id: Some(recording_config.id.into()),
				visibility: pb::scuffle::video::v1::types::Visibility::Public as i32,
				tags: None,
			},
			Ok(
				"INSERT INTO rooms (id,organization_id,transcoding_config_id,recording_config_id,visibility,stream_key,tags) VALUES ($1,$2,$3,$4,$5,$6,$7) RETURNING *",
			),
		),
	];

	for (req, expected) in test_cases {
		assert!(room::create::validate(&req).is_ok());
		let result = room::create::build_query(&req, global.db(), &access_token).await;
		assert_query_matches(result, expected);
	}

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_room_modify_qb() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let s3_bucket = create_s3_bucket(&global, access_token.organization_id, HashMap::new()).await;
	let recording_config =
		create_recording_config(&global, access_token.organization_id, s3_bucket.id, HashMap::new()).await;
	let transcoding_config = create_transcoding_config(&global, access_token.organization_id, HashMap::new()).await;
	let room = create_room(&global, access_token.organization_id).await;

	let test_cases = vec![
		(
			RoomModifyRequest {
				id: Some(room.id.into()),
				recording_config_id: Some(recording_config.id.into()),
				transcoding_config_id: Some(transcoding_config.id.into()),
				tags: Some(Tags {
					tags: vec![("example_tag".to_string(), "example_value".to_string())]
						.into_iter()
						.collect(),
				}),
				visibility: Some(pb::scuffle::video::v1::types::Visibility::Public as i32),
			},
			Ok(
				"UPDATE rooms SET transcoding_config_id = $1,recording_config_id = $2,visibility = $3,tags = $4,updated_at = NOW() WHERE id = $5 AND organization_id = $6 RETURNING *",
			),
		),
		(
			RoomModifyRequest {
				id: Some(room.id.into()),
				recording_config_id: Some(Ulid::nil().into()),
				transcoding_config_id: Some(Ulid::nil().into()),
				tags: None,
				visibility: None,
			},
			Ok(
				"UPDATE rooms SET transcoding_config_id = NULL,recording_config_id = NULL,updated_at = NOW() WHERE id = $1 AND organization_id = $2 RETURNING *",
			),
		),
	];

	for (req, expected) in test_cases {
		assert!(room::modify::validate(&req).is_ok());
		let result = room::modify::build_query(&req, global.db(), &access_token).await;
		assert_query_matches(result, expected);
	}

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_room_pair_tag_qb() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let test_cases = vec![(
		RoomTagRequest {
			id: Some(access_token.id.into()),
			tags: Some(Tags {
				tags: vec![("example_tag".to_string(), "example_value".to_string())]
					.into_iter()
					.collect(),
			}),
		},
		Ok(
			"WITH mt AS (SELECT id, tags || $1 AS new_tags, CASE WHEN tags @> $1 THEN 1 WHEN COUNT(jsonb_object_keys(tags || $1)) > $2 THEN 2 ELSE 0 END AS status FROM rooms WHERE id = $3 AND organization_id = $4 GROUP BY id, organization_id) UPDATE rooms AS t SET tags = CASE WHEN mt.status = 0 THEN mt.new_tags ELSE tags END, updated_at = CASE WHEN mt.status = 0 THEN now() ELSE updated_at END FROM mt WHERE t.id = mt.id RETURNING t.tags as tags, mt.status as status;",
		),
	)];

	for (req, expected) in test_cases {
		assert!(room::tag::validate(&req).is_ok());
		let result = room::tag::build_query(&req, &access_token);
		assert_query_matches(result, expected);
	}

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_room_pair_untag_qb() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let test_cases = vec![(
		RoomUntagRequest {
			id: Some(access_token.id.into()),
			tags: vec!["example_tag".to_string()],
		},
		Ok(
			"WITH rt AS (SELECT id, tags - $1::TEXT[] AS new_tags, CASE WHEN NOT tags ?| $1 THEN 1 ELSE 0 END AS status FROM rooms WHERE id = $2 AND organization_id = $3 GROUP BY id, organization_id) UPDATE rooms AS t SET tags = CASE WHEN rt.status = 0 THEN rt.new_tags ELSE tags END, updated_at = CASE WHEN rt.status = 0 THEN now() ELSE updated_at END FROM rt WHERE t.id = rt.id RETURNING t.tags AS tags, rt.status AS status;",
		),
	)];

	for (req, expected) in test_cases {
		assert!(room::untag::validate(&req).is_ok());
		let result = room::untag::build_query(&req, &access_token);
		assert_query_matches(result, expected);
	}

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_room_create() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let s3_bucket = create_s3_bucket(&global, access_token.organization_id, HashMap::new()).await;
	let recording_config =
		create_recording_config(&global, access_token.organization_id, s3_bucket.id, HashMap::new()).await;
	let transcoding_config = create_transcoding_config(&global, access_token.organization_id, HashMap::new()).await;

	let resp: RoomCreateResponse = process_request(
		&global,
		&access_token,
		RoomCreateRequest {
			recording_config_id: Some(recording_config.id.into()),
			transcoding_config_id: Some(transcoding_config.id.into()),
			visibility: pb::scuffle::video::v1::types::Visibility::Public as i32,
			tags: Some(Tags {
				tags: vec![("example_tag".to_string(), "example_value".to_string())]
					.into_iter()
					.collect(),
			}),
		},
	)
	.await
	.unwrap();

	assert!(resp.room.is_some(), "room should be returned");
	assert!(!resp.stream_key.is_empty(), "stream key should be returned");
	assert_eq!(
		resp.room.as_ref().unwrap().recording_config_id,
		Some(recording_config.id.into()),
		"recording config id should match"
	);
	assert_eq!(
		resp.room.as_ref().unwrap().transcoding_config_id,
		Some(transcoding_config.id.into()),
		"transcoding config id should match"
	);
	assert_eq!(
		resp.room.as_ref().unwrap().visibility,
		pb::scuffle::video::v1::types::Visibility::Public as i32,
		"visibility should match"
	);
	assert_eq!(
		resp.room.as_ref().unwrap().tags.as_ref().unwrap().tags.len(),
		1,
		"tags should match"
	);
	assert_eq!(
		resp.room.as_ref().unwrap().tags.as_ref().unwrap().tags.get("example_tag"),
		Some(&"example_value".to_string()),
		"tags should match"
	);

	let resp: RoomCreateResponse = process_request(&global, &access_token, RoomCreateRequest::default())
		.await
		.unwrap();

	assert!(resp.room.is_some(), "room should be returned");
	assert!(!resp.stream_key.is_empty(), "stream key should be returned");
	assert!(
		resp.room.as_ref().unwrap().recording_config_id.is_none(),
		"recording config id should be unset"
	);
	assert!(
		resp.room.as_ref().unwrap().transcoding_config_id.is_none(),
		"transcoding config id should be unset"
	);
	assert_eq!(
		resp.room.as_ref().unwrap().visibility,
		pb::scuffle::video::v1::types::Visibility::Public as i32,
		"visibility should default to public"
	);
	assert_eq!(
		resp.room.as_ref().unwrap().tags.as_ref().unwrap().tags.len(),
		0,
		"tags should be empty"
	);

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_room_get() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let s3_bucket = create_s3_bucket(&global, access_token.organization_id, HashMap::new()).await;
	let recording_config =
		create_recording_config(&global, access_token.organization_id, s3_bucket.id, HashMap::new()).await;
	let transcoding_config = create_transcoding_config(&global, access_token.organization_id, HashMap::new()).await;

	let rooms = vec![
		create_room(&global, access_token.organization_id).await,
		create_room(&global, access_token.organization_id).await,
		create_room(&global, access_token.organization_id).await,
		create_room(&global, access_token.organization_id).await,
		create_room(&global, access_token.organization_id).await,
		create_room(&global, access_token.organization_id).await,
	];

	let resp: RoomGetResponse = process_request(
		&global,
		&access_token,
		RoomGetRequest {
			ids: vec![rooms[0].id.into(), rooms[1].id.into(), rooms[2].id.into()],
			transcoding_config_id: None,
			recording_config_id: None,
			visibility: None,
			status: None,
			search_options: None,
		},
	)
	.await
	.unwrap();

	assert_eq!(resp.rooms.len(), 3, "should return 3 rooms");

	let resp: RoomGetResponse = process_request(
		&global,
		&access_token,
		RoomGetRequest {
			ids: vec![rooms[0].id.into(), rooms[1].id.into(), rooms[2].id.into()],
			transcoding_config_id: None,
			recording_config_id: None,
			visibility: None,
			status: None,
			search_options: Some(SearchOptions {
				limit: 1,
				reverse: true,
				after_id: Some(rooms[2].id.into()),
				tags: None,
			}),
		},
	)
	.await
	.unwrap();

	assert_eq!(resp.rooms.len(), 1, "should return 1 room");

	::utils::database::query("UPDATE rooms SET visibility = $1 WHERE id = $2 AND organization_id = $3")
		.bind(Visibility::from(pb::scuffle::video::v1::types::Visibility::Private))
		.bind(rooms[0].id)
		.bind(access_token.organization_id)
		.build()
		.execute(global.db())
		.await
		.unwrap();

	let resp: RoomGetResponse = process_request(
		&global,
		&access_token,
		RoomGetRequest {
			ids: vec![],
			transcoding_config_id: None,
			recording_config_id: None,
			visibility: Some(pb::scuffle::video::v1::types::Visibility::Private as i32),
			status: None,
			search_options: None,
		},
	)
	.await
	.unwrap();

	assert_eq!(resp.rooms.len(), 1, "should return 1 room");
	assert_eq!(resp.rooms[0].id, Some(rooms[0].id.into()), "should return the correct room");

	let resp: RoomGetResponse = process_request(
		&global,
		&access_token,
		RoomGetRequest {
			ids: vec![],
			transcoding_config_id: None,
			recording_config_id: None,
			visibility: Some(pb::scuffle::video::v1::types::Visibility::Public as i32),
			status: None,
			search_options: None,
		},
	)
	.await
	.unwrap();

	assert_eq!(resp.rooms.len(), 5, "should return 5 rooms");

	let resp: RoomGetResponse = process_request(
		&global,
		&access_token,
		RoomGetRequest {
			ids: vec![],
			transcoding_config_id: None,
			recording_config_id: None,
			visibility: None,
			status: Some(pb::scuffle::video::v1::types::RoomStatus::Ready as i32),
			search_options: None,
		},
	)
	.await
	.unwrap();

	assert_eq!(resp.rooms.len(), 0, "should return 0 rooms");

	::utils::database::query("UPDATE rooms SET status = $1 WHERE id = $2 AND organization_id = $3")
		.bind(RoomStatus::from(pb::scuffle::video::v1::types::RoomStatus::Ready))
		.bind(rooms[0].id)
		.bind(access_token.organization_id)
		.build()
		.execute(global.db())
		.await
		.unwrap();

	let resp: RoomGetResponse = process_request(
		&global,
		&access_token,
		RoomGetRequest {
			ids: vec![],
			transcoding_config_id: None,
			recording_config_id: None,
			visibility: None,
			status: Some(pb::scuffle::video::v1::types::RoomStatus::Ready as i32),
			search_options: None,
		},
	)
	.await
	.unwrap();

	assert_eq!(resp.rooms.len(), 1, "should return 1 room");
	assert_eq!(resp.rooms[0].id, Some(rooms[0].id.into()), "should return the correct room");

	::utils::database::query(
		"UPDATE rooms SET recording_config_id = $1, transcoding_config_id = $2 WHERE id = $3 AND organization_id = $4",
	)
	.bind(recording_config.id)
	.bind(transcoding_config.id)
	.bind(rooms[0].id)
	.bind(access_token.organization_id)
	.build()
	.execute(global.db())
	.await
	.unwrap();

	let resp: RoomGetResponse = process_request(
		&global,
		&access_token,
		RoomGetRequest {
			ids: vec![],
			transcoding_config_id: Some(transcoding_config.id.into()),
			recording_config_id: Some(recording_config.id.into()),
			visibility: None,
			status: None,
			search_options: None,
		},
	)
	.await
	.unwrap();

	assert_eq!(resp.rooms.len(), 1, "should return 1 room");

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_room_modify() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let s3_bucket = create_s3_bucket(&global, access_token.organization_id, HashMap::new()).await;
	let recording_config =
		create_recording_config(&global, access_token.organization_id, s3_bucket.id, HashMap::new()).await;
	let transcoding_config = create_transcoding_config(&global, access_token.organization_id, HashMap::new()).await;

	let room = create_room(&global, access_token.organization_id).await;

	let resp: RoomModifyResponse = process_request(
		&global,
		&access_token,
		RoomModifyRequest {
			id: Some(room.id.into()),
			recording_config_id: Some(recording_config.id.into()),
			transcoding_config_id: Some(transcoding_config.id.into()),
			tags: Some(Tags {
				tags: vec![("example_tag".to_string(), "example_value".to_string())]
					.into_iter()
					.collect(),
			}),
			visibility: Some(pb::scuffle::video::v1::types::Visibility::Public as i32),
		},
	)
	.await
	.unwrap();

	assert_eq!(
		resp.room.as_ref().unwrap().id,
		Some(room.id.into()),
		"should return the correct room"
	);
	assert_eq!(
		resp.room.as_ref().unwrap().recording_config_id,
		Some(recording_config.id.into()),
		"recording config id should match"
	);
	assert_eq!(
		resp.room.as_ref().unwrap().transcoding_config_id,
		Some(transcoding_config.id.into()),
		"transcoding config id should match"
	);
	assert_eq!(
		resp.room.as_ref().unwrap().visibility,
		pb::scuffle::video::v1::types::Visibility::Public as i32,
		"visibility should match"
	);
	assert_eq!(
		resp.room.as_ref().unwrap().tags.as_ref().unwrap().tags.len(),
		1,
		"tags should match"
	);
	assert_eq!(
		resp.room.as_ref().unwrap().tags.as_ref().unwrap().tags.get("example_tag"),
		Some(&"example_value".to_string()),
		"tags should match"
	);

	let resp: RoomModifyResponse = process_request(
		&global,
		&access_token,
		RoomModifyRequest {
			id: Some(room.id.into()),
			recording_config_id: Some(Ulid::nil().into()),
			transcoding_config_id: Some(Ulid::nil().into()),
			tags: None,
			visibility: None,
		},
	)
	.await
	.unwrap();

	assert_eq!(
		resp.room.as_ref().unwrap().id,
		Some(room.id.into()),
		"should return the correct room"
	);
	assert!(
		resp.room.as_ref().unwrap().recording_config_id.is_none(),
		"recording config id should be unset"
	);
	assert!(
		resp.room.as_ref().unwrap().transcoding_config_id.is_none(),
		"transcoding config id should be unset"
	);
	assert_eq!(
		resp.room.as_ref().unwrap().visibility,
		pb::scuffle::video::v1::types::Visibility::Public as i32,
		"visibility should default to public"
	);
	assert_eq!(
		resp.room.as_ref().unwrap().tags.as_ref().unwrap().tags.len(),
		1,
		"tags should be empty"
	);

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_room_tag() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let room = create_room(&global, access_token.organization_id).await;

	let resp: RoomTagResponse = process_request(
		&global,
		&access_token,
		RoomTagRequest {
			id: Some(room.id.into()),
			tags: Some(Tags {
				tags: vec![("example_tag".to_string(), "example_value".to_string())]
					.into_iter()
					.collect(),
			}),
		},
	)
	.await
	.unwrap();

	assert_eq!(resp.tags.as_ref().unwrap().tags.len(), 1, "tags should match");
	assert_eq!(
		resp.tags.as_ref().unwrap().tags.get("example_tag"),
		Some(&"example_value".to_string()),
		"tags should match"
	);

	let resp: RoomTagResponse = process_request(
		&global,
		&access_token,
		RoomTagRequest {
			id: Some(room.id.into()),
			tags: Some(Tags {
				tags: vec![("example_tag2".to_string(), "example_value2".to_string())]
					.into_iter()
					.collect(),
			}),
		},
	)
	.await
	.unwrap();

	assert_eq!(resp.tags.as_ref().unwrap().tags.len(), 2, "tags should match");
	assert_eq!(
		resp.tags.as_ref().unwrap().tags.get("example_tag"),
		Some(&"example_value".to_string()),
		"tags should match"
	);
	assert_eq!(
		resp.tags.as_ref().unwrap().tags.get("example_tag2"),
		Some(&"example_value2".to_string()),
		"tags should match"
	);

	let resp: RoomTagResponse = process_request(
		&global,
		&access_token,
		RoomTagRequest {
			id: Some(room.id.into()),
			tags: Some(Tags {
				tags: vec![("example_tag".to_string(), "new_value".to_string())]
					.into_iter()
					.collect(),
			}),
		},
	)
	.await
	.unwrap();

	assert_eq!(resp.tags.as_ref().unwrap().tags.len(), 2, "tags should match");
	assert_eq!(
		resp.tags.as_ref().unwrap().tags.get("example_tag"),
		Some(&"new_value".to_string()),
		"tags should match"
	);
	assert_eq!(
		resp.tags.as_ref().unwrap().tags.get("example_tag2"),
		Some(&"example_value2".to_string()),
		"tags should match"
	);

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_room_untag() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let room = create_room(&global, access_token.organization_id).await;

	let resp: RoomTagResponse = process_request(
		&global,
		&access_token,
		RoomTagRequest {
			id: Some(room.id.into()),
			tags: Some(Tags {
				tags: vec![("example_tag".to_string(), "example_value".to_string())]
					.into_iter()
					.collect(),
			}),
		},
	)
	.await
	.unwrap();

	assert_eq!(resp.tags.as_ref().unwrap().tags.len(), 1, "tags should match");
	assert_eq!(
		resp.tags.as_ref().unwrap().tags.get("example_tag"),
		Some(&"example_value".to_string()),
		"tags should match"
	);

	let resp: RoomUntagResponse = process_request(
		&global,
		&access_token,
		RoomUntagRequest {
			id: Some(room.id.into()),
			tags: vec!["example_tag".to_string()],
		},
	)
	.await
	.unwrap();

	assert_eq!(resp.tags.as_ref().unwrap().tags.len(), 0, "tags should match");

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_room_delete() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let room = create_room(&global, access_token.organization_id).await;

	let resp: RoomDeleteResponse = process_request(
		&global,
		&access_token,
		RoomDeleteRequest {
			ids: vec![room.id.into()],
		},
	)
	.await
	.unwrap();

	assert_eq!(resp.ids.len(), 1, "1 room should be deleted");
	assert!(resp.failed_deletes.is_empty(), "no rooms should fail to delete");

	let resp: RoomDeleteResponse = process_request(
		&global,
		&access_token,
		RoomDeleteRequest {
			ids: vec![room.id.into()],
		},
	)
	.await
	.unwrap();

	assert!(resp.ids.is_empty(), "no rooms should be deleted");
	assert_eq!(resp.failed_deletes.len(), 1, "1 room should fail to delete");
	assert_eq!(resp.failed_deletes[0].id, Some(room.id.into()), "failed delete should match");
	assert_eq!(resp.failed_deletes[0].reason, "room not found", "failed delete should match");

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_room_disconnect() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let room = create_room(&global, access_token.organization_id).await;

	let active_ingest_connection_id = Ulid::new();

	::utils::database::query(
		"UPDATE rooms SET status = $1, active_ingest_connection_id = $2 WHERE id = $3 AND organization_id = $4",
	)
	.bind(RoomStatus::from(pb::scuffle::video::v1::types::RoomStatus::Ready))
	.bind(active_ingest_connection_id)
	.bind(room.id)
	.bind(access_token.organization_id)
	.build()
	.execute(global.db())
	.await
	.unwrap();

	let mut subscription = global
		.nats()
		.subscribe(video_common::keys::ingest_disconnect(active_ingest_connection_id))
		.await
		.unwrap();

	let resp: RoomDisconnectResponse = process_request(
		&global,
		&access_token,
		RoomDisconnectRequest {
			ids: vec![room.id.into()],
		},
	)
	.await
	.unwrap();

	assert_eq!(resp.ids.len(), 1, "1 room should be deleted");
	assert!(resp.failed_disconnects.is_empty(), "no rooms should fail to disconnect");

	let msg = subscription
		.next()
		.timeout(std::time::Duration::from_secs(1))
		.await
		.unwrap()
		.unwrap();

	assert_eq!(
		msg.subject.as_str(),
		video_common::keys::ingest_disconnect(active_ingest_connection_id),
		"should receive disconnect message"
	);
	assert!(msg.payload.is_empty(), "payload should be empty");

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_room_reset_keys() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let room = create_room(&global, access_token.organization_id).await;

	let resp: RoomResetKeyResponse = process_request(
		&global,
		&access_token,
		RoomResetKeyRequest {
			ids: vec![room.id.into()],
		},
	)
	.await
	.unwrap();

	assert_eq!(resp.rooms.len(), 1, "1 room should be reset");
	assert!(resp.failed_resets.is_empty(), "no rooms should fail to reset keys");

	let key: String = ::utils::database::query("SELECT stream_key FROM rooms WHERE id = $1 AND organization_id = $2")
		.bind(room.id)
		.bind(access_token.organization_id)
		.build_query_single_scalar()
		.fetch_one(global.db())
		.await
		.unwrap();

	assert_ne!(key, room.stream_key, "stream key should be different");
	assert_eq!(resp.rooms[0].id, Some(room.id.into()), "room should match");
	assert_eq!(resp.rooms[0].key, key, "room should match");

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_room_boilerplate() {
	let (global, handler, main_access_token) = scuffle_utilssetup(Default::default()).await;

	let no_scopes_token =
		scuffle_utilscreate_access_token(&global, &main_access_token.organization_id, vec![], HashMap::new()).await;

	let room = create_room(&global, main_access_token.organization_id).await;

	let server = RoomServer::<GlobalState>::new();

	use pb::scuffle::video::v1::room_server::Room as _;

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
			RoomGetRequest {
				ids: vec![room.id.into()],
				..Default::default()
			},
		))
		.await
		.unwrap();
	assert_eq!(response.get_ref().rooms.len(), 1);
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
		.get(build_request(&global, &no_scopes_token, RoomGetRequest::default()))
		.await
		.unwrap_err();
	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: room:read");

	let response = server
		.create(build_request(
			&global,
			&main_access_token,
			RoomCreateRequest { ..Default::default() },
		))
		.await
		.unwrap();
	assert!(response.get_ref().room.is_some());
	assert!(!response.get_ref().stream_key.is_empty());
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
		.create(build_request(&global, &no_scopes_token, RoomCreateRequest::default()))
		.await
		.unwrap_err();

	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: room:create");

	let response = server
		.modify(build_request(
			&global,
			&main_access_token,
			RoomModifyRequest {
				id: Some(room.id.into()),
				tags: Some(Tags {
					tags: vec![("example_tag".to_string(), "example_value".to_string())]
						.into_iter()
						.collect(),
				}),
				..Default::default()
			},
		))
		.await
		.unwrap();

	assert!(response.get_ref().room.is_some());
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
		.modify(build_request(&global, &no_scopes_token, RoomModifyRequest::default()))
		.await
		.unwrap_err();

	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: room:modify");

	let response = server
		.tag(build_request(
			&global,
			&main_access_token,
			RoomTagRequest {
				id: Some(room.id.into()),
				tags: Some(Tags {
					tags: vec![("example_tag".to_string(), "example_value".to_string())]
						.into_iter()
						.collect(),
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
	assert_eq!(
		response.metadata().len(),
		2,
		"rate limit headers should be the only headers set"
	);

	let response = server
		.tag(build_request(&global, &no_scopes_token, RoomTagRequest::default()))
		.await
		.unwrap_err();

	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: room:modify");

	let response = server
		.untag(build_request(
			&global,
			&main_access_token,
			RoomUntagRequest {
				id: Some(room.id.into()),
				tags: vec!["example_tag".to_string()],
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
	assert_eq!(
		response.metadata().len(),
		2,
		"rate limit headers should be the only headers set"
	);

	let response = server
		.untag(build_request(&global, &no_scopes_token, RoomUntagRequest::default()))
		.await
		.unwrap_err();

	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: room:modify");

	let response = server
		.reset_key(build_request(
			&global,
			&main_access_token,
			RoomResetKeyRequest {
				ids: vec![room.id.into()],
			},
		))
		.await
		.unwrap();

	assert_eq!(response.get_ref().rooms.len(), 1);
	assert!(response.get_ref().failed_resets.is_empty());
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
		.reset_key(build_request(&global, &no_scopes_token, RoomResetKeyRequest::default()))
		.await
		.unwrap_err();

	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: room:modify");

	let response = server
		.disconnect(build_request(
			&global,
			&main_access_token,
			RoomDisconnectRequest {
				ids: vec![room.id.into()],
			},
		))
		.await
		.unwrap();

	assert!(response.get_ref().ids.is_empty(), "room should fail to disconnect");
	assert_eq!(
		response.get_ref().failed_disconnects.len(),
		1,
		"room should fail to disconnect"
	);
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
		.disconnect(build_request(&global, &no_scopes_token, RoomDisconnectRequest::default()))
		.await
		.unwrap_err();

	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: room:modify");

	let response = server
		.delete(build_request(
			&global,
			&main_access_token,
			RoomDeleteRequest {
				ids: vec![room.id.into()],
			},
		))
		.await
		.unwrap();

	assert_eq!(response.get_ref().ids.len(), 1);
	assert!(response.get_ref().failed_deletes.is_empty());
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
		.delete(build_request(&global, &no_scopes_token, RoomDeleteRequest::default()))
		.await
		.unwrap_err();

	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: room:delete");

	scuffle_utilsteardown(global, handler).await;
}
