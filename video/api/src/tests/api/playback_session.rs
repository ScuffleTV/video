use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;

use binary_helper::global::GlobalDb;
use chrono::Utc;
use pb::scuffle::video::v1::types::{playback_session_target, PlaybackSessionTarget, SearchOptions};
use pb::scuffle::video::v1::{
	playback_session_count_request, PlaybackSessionCountRequest, PlaybackSessionCountResponse, PlaybackSessionGetRequest,
	PlaybackSessionGetResponse, PlaybackSessionRevokeRequest, PlaybackSessionRevokeResponse,
};
use rand::{Rng, SeedableRng};
use ulid::Ulid;
use video_common::database::AccessToken;

use crate::api::playback_session::{self, PlaybackSessionServer};
use crate::tests::api::utils::{
	assert_query_matches, create_playback_session, create_recording, create_room, create_s3_bucket, process_request,
};
use crate::tests::global::GlobalState;
use crate::tests::utils::{self, teardown};

#[tokio::test]
async fn test_playback_session_count_qb() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let test_cases = vec![
		(
			PlaybackSessionCountRequest {
				filter: Some(playback_session_count_request::Filter::UserId("test".to_string())),
			},
			Ok(
				"SELECT COUNT(*) AS total_count, COUNT(DISTINCT (recording_id, room_id)) AS deduped FROM playback_sessions WHERE user_id = $1 AND organization_id = $2",
			),
		),
		(
			PlaybackSessionCountRequest {
				filter: Some(playback_session_count_request::Filter::Target(PlaybackSessionTarget {
					target: Some(playback_session_target::Target::RoomId(Ulid::new().into())),
				})),
			},
			Ok(
				"SELECT COUNT(*) AS total_count, COUNT(DISTINCT COALESCE(user_id, ip_address::text)) AS deduped FROM playback_sessions WHERE organization_id = $1 AND room_id = $2 AND recording_id IS NULL",
			),
		),
		(
			PlaybackSessionCountRequest {
				filter: Some(playback_session_count_request::Filter::Target(PlaybackSessionTarget {
					target: Some(playback_session_target::Target::RecordingId(Ulid::new().into())),
				})),
			},
			Ok(
				"SELECT COUNT(*) AS total_count, COUNT(DISTINCT COALESCE(user_id, ip_address::text)) AS deduped FROM playback_sessions WHERE organization_id = $1 AND recording_id = $2 AND room_id IS NULL",
			),
		),
	];

	for (req, expected) in test_cases {
		let result = playback_session::count::build_query(&req, &access_token);
		assert_query_matches(result, expected);
	}

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_playback_session_get_qb() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let test_cases = vec![
		(
			PlaybackSessionGetRequest {
				user_id: None,
				authorized: None,
				ids: vec![],
				search_options: None,
				target: None,
				ip_address: None,
				playback_key_pair_id: None,
			},
			Ok("SELECT * FROM playback_sessions WHERE organization_id = $1 ORDER BY id ASC LIMIT 100"),
		),
		(
			PlaybackSessionGetRequest {
				authorized: Some(true),
				ids: vec![],
				search_options: None,
				target: None,
				user_id: None,
				ip_address: None,
				playback_key_pair_id: None,
			},
			Ok(
				"SELECT * FROM playback_sessions WHERE organization_id = $1 AND playback_key_pair_id IS NOT NULL ORDER BY id ASC LIMIT 100",
			),
		),
		(
			PlaybackSessionGetRequest {
				authorized: Some(true),
				ids: vec![],
				search_options: Some(SearchOptions {
					limit: 10,
					reverse: false,
					tags: None,
					after_id: None,
				}),
				target: None,
				user_id: None,
				ip_address: None,
				playback_key_pair_id: None,
			},
			Ok(
				"SELECT * FROM playback_sessions WHERE organization_id = $1 AND playback_key_pair_id IS NOT NULL ORDER BY id ASC LIMIT $2",
			),
		),
		(
			PlaybackSessionGetRequest {
				authorized: Some(true),
				ids: vec![],
				search_options: Some(SearchOptions {
					limit: 10,
					reverse: false,
					tags: None,
					after_id: None,
				}),
				target: Some(PlaybackSessionTarget {
					target: Some(playback_session_target::Target::RecordingId(Ulid::new().into())),
				}),
				user_id: None,
				ip_address: None,
				playback_key_pair_id: None,
			},
			Ok(
				"SELECT * FROM playback_sessions WHERE organization_id = $1 AND playback_key_pair_id IS NOT NULL AND recording_id = $2 ORDER BY id ASC LIMIT $3",
			),
		),
		(
			PlaybackSessionGetRequest {
				authorized: Some(true),
				ids: vec![],
				search_options: Some(SearchOptions {
					limit: 10,
					reverse: false,
					tags: None,
					after_id: None,
				}),
				target: Some(PlaybackSessionTarget {
					target: Some(playback_session_target::Target::RoomId(Ulid::new().into())),
				}),
				user_id: Some("test".to_string()),
				ip_address: None,
				playback_key_pair_id: None,
			},
			Ok(
				"SELECT * FROM playback_sessions WHERE organization_id = $1 AND user_id = $2 AND playback_key_pair_id IS NOT NULL AND room_id = $3 ORDER BY id ASC LIMIT $4",
			),
		),
		(
			PlaybackSessionGetRequest {
				authorized: Some(true),
				ids: vec![Ulid::new().into()],
				search_options: Some(SearchOptions {
					limit: 10,
					reverse: false,
					tags: None,
					after_id: Some(Ulid::new().into()),
				}),
				target: Some(PlaybackSessionTarget {
					target: Some(playback_session_target::Target::RoomId(Ulid::new().into())),
				}),
				user_id: Some("test".to_string()),
				ip_address: None,
				playback_key_pair_id: None,
			},
			Ok(
				"SELECT * FROM playback_sessions WHERE organization_id = $1 AND id = ANY($2) AND user_id = $3 AND playback_key_pair_id IS NOT NULL AND room_id = $4 AND id > $5 ORDER BY id ASC LIMIT $6",
			),
		),
		(
			PlaybackSessionGetRequest {
				authorized: Some(true),
				ids: vec![Ulid::new().into()],
				search_options: Some(SearchOptions {
					limit: 10,
					reverse: false,
					tags: None,
					after_id: Some(Ulid::new().into()),
				}),
				target: Some(PlaybackSessionTarget {
					target: Some(playback_session_target::Target::RoomId(Ulid::new().into())),
				}),
				user_id: Some("test".to_string()),
				ip_address: Some("127.2.1.1".into()),
				playback_key_pair_id: Some(Ulid::new().into()),
			},
			Ok(
				"SELECT * FROM playback_sessions WHERE organization_id = $1 AND id = ANY($2) AND user_id = $3 AND playback_key_pair_id = $4 AND ip_address = $5 AND room_id = $6 AND id > $7 ORDER BY id ASC LIMIT $8",
			),
		),
	];

	for (req, expected) in test_cases {
		let result = playback_session::get::build_query(&req, &access_token);
		assert_query_matches(result, expected);
	}

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_playback_session_count() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let s3_bucket = create_s3_bucket(&global, access_token.organization_id, HashMap::new()).await;
	let recording = create_recording(
		&global,
		access_token.organization_id,
		s3_bucket.id,
		None,
		None,
		HashMap::new(),
	)
	.await;
	let room = create_room(&global, access_token.organization_id).await;

	let mut rand = rand_chacha::ChaCha8Rng::seed_from_u64(1000);

	create_playback_session(
		&global,
		access_token.organization_id,
		(0..100).flat_map(|i| {
			let ip = IpAddr::from(rand.gen::<u32>().to_be_bytes());
			[
				(
					Some(room.id),
					None,
					Some(format!("test-{i}")),
					IpAddr::from(rand.gen::<u32>().to_be_bytes()),
				),
				(
					Some(room.id),
					None,
					Some(format!("test-{i}")),
					IpAddr::from(rand.gen::<u32>().to_be_bytes()),
				),
				(Some(room.id), None, None, IpAddr::from(rand.gen::<u32>().to_be_bytes())),
				(None, Some(recording.id), None, ip),
				(None, Some(recording.id), None, ip),
				(None, Some(recording.id), Some(format!("test-{i}")), ip),
			]
		}),
	)
	.await;

	let response: PlaybackSessionCountResponse = process_request(
		&global,
		&access_token,
		PlaybackSessionCountRequest {
			filter: Some(playback_session_count_request::Filter::UserId("test-0".to_string())),
		},
	)
	.await
	.expect("counting should be successful");

	assert_eq!(response.count, 3);
	assert_eq!(response.deduplicated_count, 2);

	let response: PlaybackSessionCountResponse = process_request(
		&global,
		&access_token,
		PlaybackSessionCountRequest {
			filter: Some(playback_session_count_request::Filter::Target(PlaybackSessionTarget {
				target: Some(playback_session_target::Target::RoomId(room.id.into())),
			})),
		},
	)
	.await
	.expect("counting should be successful");

	assert_eq!(response.count, 300);
	assert_eq!(response.deduplicated_count, 200);

	let response: PlaybackSessionCountResponse = process_request(
		&global,
		&access_token,
		PlaybackSessionCountRequest {
			filter: Some(playback_session_count_request::Filter::Target(PlaybackSessionTarget {
				target: Some(playback_session_target::Target::RecordingId(recording.id.into())),
			})),
		},
	)
	.await
	.expect("counting should be successful");

	assert_eq!(response.count, 300);
	assert_eq!(response.deduplicated_count, 200);

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_playback_session_revoke() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let s3_bucket = create_s3_bucket(&global, access_token.organization_id, HashMap::new()).await;
	let recording = create_recording(
		&global,
		access_token.organization_id,
		s3_bucket.id,
		None,
		None,
		HashMap::new(),
	)
	.await;
	let room = create_room(&global, access_token.organization_id).await;

	let mut rand = rand_chacha::ChaCha8Rng::seed_from_u64(1000);

	create_playback_session(
		&global,
		access_token.organization_id,
		(0..100).flat_map(|i| {
			[
				(Some(room.id), None, None, IpAddr::from(rand.gen::<u32>().to_be_bytes())),
				(
					Some(room.id),
					None,
					Some(format!("test-{i}")),
					IpAddr::from(rand.gen::<u32>().to_be_bytes()),
				),
				(None, Some(recording.id), None, IpAddr::from(rand.gen::<u32>().to_be_bytes())),
				(
					None,
					Some(recording.id),
					Some(format!("test-{i}")),
					IpAddr::from(rand.gen::<u32>().to_be_bytes()),
				),
			]
		}),
	)
	.await;

	let response: PlaybackSessionRevokeResponse = process_request(
		&global,
		&access_token,
		PlaybackSessionRevokeRequest {
			target: Some(PlaybackSessionTarget {
				target: Some(playback_session_target::Target::RoomId(room.id.into())),
			}),
			ids: vec![],
			before: None,
			user_id: None,
			authorized: None,
		},
	)
	.await
	.expect("revoking should be successful");

	assert_eq!(response.revoked, 200);

	let revoke_before: chrono::DateTime<chrono::Utc> = ::utils::database::query("SELECT revoke_before FROM playback_session_revocations WHERE organization_id = $1 AND room_id = $2 AND recording_id IS NULL AND user_id IS NULL").bind(access_token.organization_id).bind(room.id).build_query_single_scalar().fetch_one(global.db()).await.expect("fetching revocations should be successful");

	// Assert that the revoke_before is within 5 seconds of now
	assert!(
		revoke_before > chrono::Utc::now() - chrono::Duration::seconds(5),
		"revoke_before should be within 5 seconds of now"
	);

	let response: PlaybackSessionRevokeResponse = process_request(
		&global,
		&access_token,
		PlaybackSessionRevokeRequest {
			target: Some(PlaybackSessionTarget {
				target: Some(playback_session_target::Target::RecordingId(recording.id.into())),
			}),
			ids: vec![],
			before: None,
			user_id: None,
			authorized: Some(false),
		},
	)
	.await
	.expect("revoking should be successful");

	assert_eq!(response.revoked, 100);

	let revoke_before: chrono::DateTime<chrono::Utc> = ::utils::database::query("SELECT revoke_before FROM playback_session_revocations WHERE organization_id = $1 AND room_id IS NULL AND recording_id = $2 AND user_id IS NULL").bind(access_token.organization_id).bind(recording.id).build_query_single_scalar().fetch_one(global.db()).await.expect("fetching revocations should be successful");

	assert!(
		revoke_before > chrono::Utc::now() - chrono::Duration::seconds(5),
		"revoke_before should be within 5 seconds of now"
	);

	let response: PlaybackSessionRevokeResponse = process_request(
		&global,
		&access_token,
		PlaybackSessionRevokeRequest {
			target: Some(PlaybackSessionTarget {
				target: Some(playback_session_target::Target::RecordingId(recording.id.into())),
			}),
			ids: vec![],
			before: None,
			user_id: Some("test-0".to_string()),
			authorized: None,
		},
	)
	.await
	.expect("revoking should be successful");

	assert_eq!(response.revoked, 1);

	let revoke_before: chrono::DateTime<chrono::Utc> = ::utils::database::query("SELECT revoke_before FROM playback_session_revocations WHERE organization_id = $1 AND room_id IS NULL AND recording_id = $2 AND user_id = $3").bind(access_token.organization_id).bind(recording.id).bind("test-0").build_query_single_scalar().fetch_one(global.db()).await.expect("fetching revocations should be successful");

	assert!(
		revoke_before > chrono::Utc::now() - chrono::Duration::seconds(5),
		"revoke_before should be within 5 seconds of now"
	);

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_playback_session_revoke_2() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let s3_bucket = create_s3_bucket(&global, access_token.organization_id, HashMap::new()).await;
	let recording = create_recording(
		&global,
		access_token.organization_id,
		s3_bucket.id,
		None,
		None,
		HashMap::new(),
	)
	.await;
	let room = create_room(&global, access_token.organization_id).await;

	let mut rand = rand_chacha::ChaCha8Rng::seed_from_u64(1000);

	let sessions = create_playback_session(
		&global,
		access_token.organization_id,
		(0..100).flat_map(|i| {
			[
				(
					Some(room.id),
					None,
					Some(format!("test-{i}")),
					IpAddr::from(rand.gen::<u32>().to_be_bytes()),
				),
				(Some(room.id), None, None, IpAddr::from(rand.gen::<u32>().to_be_bytes())),
				(None, Some(recording.id), None, IpAddr::from(rand.gen::<u32>().to_be_bytes())),
				(
					None,
					Some(recording.id),
					Some(format!("test-{i}")),
					IpAddr::from(rand.gen::<u32>().to_be_bytes()),
				),
			]
		}),
	)
	.await;

	let response: PlaybackSessionRevokeResponse = process_request(
		&global,
		&access_token,
		PlaybackSessionRevokeRequest {
			target: None,
			ids: sessions.iter().take(100).map(|s| s.id.into()).collect(),
			before: Some(Utc::now().timestamp_millis()),
			user_id: None,
			authorized: Some(true),
		},
	)
	.await
	.expect("revoking should be successful");

	// Half of them are authorized, so 50 should be revoked
	assert_eq!(response.revoked, 50);

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_playback_session_get() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let s3_bucket = create_s3_bucket(&global, access_token.organization_id, HashMap::new()).await;
	let recording = create_recording(
		&global,
		access_token.organization_id,
		s3_bucket.id,
		None,
		None,
		HashMap::new(),
	)
	.await;
	let room = create_room(&global, access_token.organization_id).await;

	let mut rand = rand_chacha::ChaCha8Rng::seed_from_u64(1000);

	create_playback_session(
		&global,
		access_token.organization_id,
		(0..100).flat_map(|i| {
			[
				(
					Some(room.id),
					None,
					Some(format!("test-{i}")),
					IpAddr::from(rand.gen::<u32>().to_be_bytes()),
				),
				(Some(room.id), None, None, IpAddr::from(rand.gen::<u32>().to_be_bytes())),
				(None, Some(recording.id), None, IpAddr::from(rand.gen::<u32>().to_be_bytes())),
				(
					None,
					Some(recording.id),
					Some(format!("test-{i}")),
					IpAddr::from(rand.gen::<u32>().to_be_bytes()),
				),
			]
		}),
	)
	.await;

	let expected_result_count = [100, 100, 100, 100, 0];

	let mut after_id = None;

	for expected_result in expected_result_count {
		let response: PlaybackSessionGetResponse = process_request(
			&global,
			&access_token,
			PlaybackSessionGetRequest {
				target: None,
				authorized: None,
				ids: vec![],
				ip_address: None,
				playback_key_pair_id: None,
				search_options: Some(SearchOptions {
					limit: 100,
					reverse: false,
					tags: None,
					after_id,
				}),
				user_id: None,
			},
		)
		.await
		.unwrap();

		assert_eq!(
			response.sessions.len(),
			expected_result,
			"expected {} results, got {}",
			expected_result,
			response.sessions.len()
		);
		after_id = response.sessions.last().and_then(|s| s.id);
	}

	teardown(global, handler).await;
}

#[tokio::test]
async fn test_playback_session_boiler_plate() {
	let (global, handler, main_access_token) = scuffle_utilssetup(Default::default()).await;

	let no_scopes_token =
		scuffle_utilscreate_access_token(&global, &main_access_token.organization_id, vec![], HashMap::new()).await;

	let server = PlaybackSessionServer::<GlobalState>::new();

	use pb::scuffle::video::v1::playback_session_server::PlaybackSession as _;

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
			PlaybackSessionGetRequest {
				ids: vec![],
				authorized: None,
				search_options: None,
				target: None,
				user_id: None,
				ip_address: None,
				playback_key_pair_id: None,
			},
		))
		.await
		.unwrap();
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
		.get(build_request(&global, &no_scopes_token, PlaybackSessionGetRequest::default()))
		.await
		.unwrap_err();
	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: playback_session:read");

	let response = server
		.count(build_request(
			&global,
			&main_access_token,
			PlaybackSessionCountRequest {
				filter: Some(playback_session_count_request::Filter::UserId("test".to_string())),
			},
		))
		.await
		.unwrap();
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
		.count(build_request(
			&global,
			&no_scopes_token,
			PlaybackSessionCountRequest::default(),
		))
		.await
		.unwrap_err();
	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: playback_session:read");

	let response = server
		.revoke(build_request(
			&global,
			&main_access_token,
			PlaybackSessionRevokeRequest {
				target: None,
				ids: vec![],
				before: None,
				user_id: None,
				authorized: None,
			},
		))
		.await
		.unwrap();

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
		.revoke(build_request(
			&global,
			&no_scopes_token,
			PlaybackSessionRevokeRequest::default(),
		))
		.await
		.unwrap_err();

	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: playback_session:delete");

	scuffle_utilsteardown(global, handler).await;
}
