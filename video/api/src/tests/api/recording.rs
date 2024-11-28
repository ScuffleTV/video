use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use ::utils::prelude::FutureTimeout;
use binary_helper::global::GlobalNats;
use futures_util::StreamExt;
use pb::ext::UlidExt;
use pb::scuffle::video::v1::types::{Tags, Visibility};
use pb::scuffle::video::v1::{
	RecordingDeleteRequest, RecordingDeleteResponse, RecordingGetRequest, RecordingGetResponse, RecordingModifyRequest,
	RecordingModifyResponse, RecordingTagRequest, RecordingTagResponse, RecordingUntagRequest, RecordingUntagResponse,
};
use ulid::Ulid;
use video_common::database::{AccessToken, Rendition};

use crate::api::recording::RecordingServer;
use crate::config::ApiConfig;
use crate::tests::api::utils::{
	create_recording, create_recording_config, create_recording_segment, create_recording_thumbnail, create_room,
	create_s3_bucket, process_request,
};
use crate::tests::global::GlobalState;
use crate::tests::utils;

#[tokio::test]
async fn test_recording_get() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let s3_bucket = create_s3_bucket(&global, access_token.organization_id, HashMap::new()).await;
	let room = create_room(&global, access_token.organization_id).await;
	let recording_config =
		create_recording_config(&global, access_token.organization_id, s3_bucket.id, HashMap::new()).await;
	let recording = create_recording(
		&global,
		access_token.organization_id,
		s3_bucket.id,
		Some(room.id),
		Some(recording_config.id),
		HashMap::new(),
	)
	.await;
	let recording2 = create_recording(
		&global,
		access_token.organization_id,
		s3_bucket.id,
		None,
		None,
		HashMap::new(),
	)
	.await;

	let resp: RecordingGetResponse = process_request(
		&global,
		&access_token,
		RecordingGetRequest {
			ids: vec![recording.id.into(), recording2.id.into()],
			deleted: Some(false),
			visibility: Some(Visibility::Public.into()),
			recording_config_id: Some(recording_config.id.into()),
			room_id: Some(room.id.into()),
			s3_bucket_id: Some(s3_bucket.id.into()),
			search_options: None,
		},
	)
	.await
	.unwrap();
	assert_eq!(resp.recordings.len(), 1, "expected 1 recording");
	assert_eq!(
		resp.recordings[0].id.into_ulid(),
		recording.id,
		"expected recording id to match"
	);

	let resp: RecordingGetResponse = process_request(&global, &access_token, RecordingGetRequest::default())
		.await
		.unwrap();
	assert_eq!(resp.recordings.len(), 2, "expected 2 recording");

	let resp: RecordingGetResponse = process_request(
		&global,
		&access_token,
		RecordingGetRequest {
			deleted: Some(true),
			..Default::default()
		},
	)
	.await
	.unwrap();
	assert_eq!(resp.recordings.len(), 0, "expected 0 recording");

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_recording_modify() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let s3_bucket = create_s3_bucket(&global, access_token.organization_id, HashMap::new()).await;
	let room = create_room(&global, access_token.organization_id).await;
	let recording_config =
		create_recording_config(&global, access_token.organization_id, s3_bucket.id, HashMap::new()).await;
	let recording = create_recording(
		&global,
		access_token.organization_id,
		s3_bucket.id,
		None,
		None,
		HashMap::new(),
	)
	.await;

	let resp: RecordingModifyResponse = process_request(
		&global,
		&access_token,
		RecordingModifyRequest {
			id: Some(recording.id.into()),
			recording_config_id: None,
			room_id: Some(room.id.into()),
			visibility: Some(Visibility::Private.into()),
			tags: None,
		},
	)
	.await
	.unwrap();
	assert_eq!(
		resp.recording.as_ref().unwrap().id.into_ulid(),
		recording.id,
		"expected ids to match"
	);
	assert_eq!(
		resp.recording.as_ref().unwrap().room_id.into_ulid(),
		room.id,
		"expected room id to be set"
	);
	assert_eq!(
		resp.recording.as_ref().unwrap().visibility,
		Visibility::Private as i32,
		"expected visibility to be private"
	);
	assert!(
		resp.recording.as_ref().unwrap().recording_config_id.is_none(),
		"expected recording config id to be unset"
	);
	assert_eq!(
		resp.recording.as_ref().unwrap().tags.as_ref().unwrap().tags.len(),
		0,
		"expected 0 tags"
	);

	let resp: RecordingModifyResponse = process_request(
		&global,
		&access_token,
		RecordingModifyRequest {
			id: Some(recording.id.into()),
			recording_config_id: Some(recording_config.id.into()),
			room_id: None,
			visibility: None,
			tags: Some(Tags {
				tags: vec![("test".to_string(), "test".to_string())].into_iter().collect(),
			}),
		},
	)
	.await
	.unwrap();
	assert_eq!(
		resp.recording.as_ref().unwrap().id.into_ulid(),
		recording.id,
		"expected ids to match"
	);
	assert_eq!(
		resp.recording.as_ref().unwrap().room_id.into_ulid(),
		room.id,
		"expected room id to be set"
	);
	assert_eq!(
		resp.recording.as_ref().unwrap().visibility,
		Visibility::Private as i32,
		"expected visibility to be private"
	);
	assert_eq!(
		resp.recording.as_ref().unwrap().recording_config_id.into_ulid(),
		recording_config.id,
		"expected recording config id to be set"
	);
	assert_eq!(
		resp.recording.as_ref().unwrap().tags.as_ref().unwrap().tags.len(),
		1,
		"expected 1 tags"
	);
	assert_eq!(
		resp.recording
			.as_ref()
			.unwrap()
			.tags
			.as_ref()
			.unwrap()
			.tags
			.get("test")
			.unwrap(),
		"test",
		"expected tag to match"
	);

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_recording_tag() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let s3_bucket = create_s3_bucket(&global, access_token.organization_id, HashMap::new()).await;
	let room = create_room(&global, access_token.organization_id).await;
	let recording_config =
		create_recording_config(&global, access_token.organization_id, s3_bucket.id, HashMap::new()).await;
	let recording = create_recording(
		&global,
		access_token.organization_id,
		s3_bucket.id,
		Some(room.id),
		Some(recording_config.id),
		HashMap::new(),
	)
	.await;

	let resp: RecordingTagResponse = process_request(
		&global,
		&access_token,
		RecordingTagRequest {
			id: Some(recording.id.into()),
			tags: Some(Tags {
				tags: vec![("test".to_string(), "test".to_string())].into_iter().collect(),
			}),
		},
	)
	.await
	.unwrap();
	assert_eq!(resp.tags.as_ref().unwrap().tags.len(), 1, "expected 1 tags");
	assert_eq!(
		resp.tags.as_ref().unwrap().tags.get("test").unwrap(),
		"test",
		"expected 1 tags"
	);

	let resp: RecordingTagResponse = process_request(
		&global,
		&access_token,
		RecordingTagRequest {
			id: Some(recording.id.into()),
			tags: Some(Tags {
				tags: vec![("test2".to_string(), "test2".to_string())].into_iter().collect(),
			}),
		},
	)
	.await
	.unwrap();

	assert_eq!(resp.tags.as_ref().unwrap().tags.len(), 2, "expected 2 tags");
	assert_eq!(
		resp.tags.as_ref().unwrap().tags.get("test").unwrap(),
		"test",
		"expected 1 tags"
	);
	assert_eq!(
		resp.tags.as_ref().unwrap().tags.get("test2").unwrap(),
		"test2",
		"expected 1 tags"
	);

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_recording_untag() {
	let (global, handler, access_token) = scuffle_utilssetup(Default::default()).await;

	let s3_bucket = create_s3_bucket(&global, access_token.organization_id, HashMap::new()).await;
	let room = create_room(&global, access_token.organization_id).await;
	let recording_config =
		create_recording_config(&global, access_token.organization_id, s3_bucket.id, HashMap::new()).await;
	let recording = create_recording(
		&global,
		access_token.organization_id,
		s3_bucket.id,
		Some(room.id),
		Some(recording_config.id),
		vec![
			("test".to_string(), "test".to_string()),
			("test2".to_string(), "test2".to_string()),
		]
		.into_iter()
		.collect(),
	)
	.await;

	let resp: RecordingUntagResponse = process_request(
		&global,
		&access_token,
		RecordingUntagRequest {
			id: Some(recording.id.into()),
			tags: vec!["test".to_string()],
		},
	)
	.await
	.unwrap();
	assert_eq!(resp.tags.as_ref().unwrap().tags.len(), 1, "expected 1 tags");
	assert_eq!(
		resp.tags.as_ref().unwrap().tags.get("test2").unwrap(),
		"test2",
		"expected 1 tags"
	);

	let resp: RecordingUntagResponse = process_request(
		&global,
		&access_token,
		RecordingUntagRequest {
			id: Some(recording.id.into()),
			tags: vec!["test2".to_string()],
		},
	)
	.await
	.unwrap();

	assert_eq!(resp.tags.as_ref().unwrap().tags.len(), 0, "expected 0 tags");

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_recording_delete() {
	let recording_delete_stream = Ulid::new().to_string();

	let (global, handler, access_token) = scuffle_utilssetup(ApiConfig {
		recording_delete_stream: recording_delete_stream.clone(),
		..Default::default()
	})
	.await;

	let s3_bucket = create_s3_bucket(&global, access_token.organization_id, HashMap::new()).await;
	let room = create_room(&global, access_token.organization_id).await;
	let recording_config =
		create_recording_config(&global, access_token.organization_id, s3_bucket.id, HashMap::new()).await;
	let recording = create_recording(
		&global,
		access_token.organization_id,
		s3_bucket.id,
		Some(room.id),
		Some(recording_config.id),
		HashMap::new(),
	)
	.await;

	let mut thumbnails = create_recording_thumbnail(
		&global,
		access_token.organization_id,
		recording.id,
		(0..4_320).map(|i| (i, i as f32 * 5.0)),
	)
	.await
	.into_iter()
	.map(|t| (t.id, t.idx))
	.collect::<HashSet<_>>();

	let mut segments = create_recording_segment(
		&global,
		access_token.organization_id,
		recording.id,
		Rendition::variants()
			.into_iter()
			.flat_map(|rendition| (0..10_800).map(move |i| (rendition, i, i as f32 * 2.0, i as f32 * 2.0 + 2.0))),
	)
	.await
	.into_iter()
	.map(|s| (s.rendition, s.id, s.idx))
	.collect::<HashSet<_>>();

	let mut stream_listener = global.nats().subscribe(recording_delete_stream).await.unwrap();

	let resp: RecordingDeleteResponse = process_request(
		&global,
		&access_token,
		RecordingDeleteRequest {
			ids: vec![recording.id.into()],
		},
	)
	.await
	.unwrap();

	assert_eq!(resp.ids.len(), 1, "expected 1 id");
	assert_eq!(resp.ids[0].into_ulid(), recording.id, "expected id to match");
	assert_eq!(resp.failed_deletes.len(), 0, "expected 0 failed deletes");

	let mut count = 0;

	while let Ok(Some(msg)) = stream_listener.next().timeout(Duration::from_millis(100)).await {
		count += 1;

		let msg: pb::scuffle::video::internal::events::RecordingDeleteBatchTask =
			prost::Message::decode(msg.payload).unwrap();

		assert_eq!(msg.recording_id.into_ulid(), recording.id, "expected recording id to match");
		assert_eq!(msg.s3_bucket_id.into_ulid(), s3_bucket.id, "expected s3 bucket id to match");

		assert!(
			msg.objects.len() <= 1000,
			"expected less than equal to 1000 objects per message"
		);
		assert!(!msg.objects.is_empty(), "expected at least 1 object per message");

		match msg.objects_type.unwrap() {
			pb::scuffle::video::internal::events::recording_delete_batch_task::ObjectsType::Segments(rendition) => {
				let rendition = Rendition::from(pb::scuffle::video::v1::types::Rendition::try_from(rendition).unwrap());

				for obj in msg.objects {
					assert!(
						segments.remove(&(rendition, obj.object_id.into_ulid(), obj.index)),
						"expected segment to be deleted"
					)
				}
			}
			pb::scuffle::video::internal::events::recording_delete_batch_task::ObjectsType::Thumbnails(_) => {
				for obj in msg.objects {
					assert!(
						thumbnails.remove(&(obj.object_id.into_ulid(), obj.index)),
						"expected thumbnail to be deleted"
					)
				}
			}
		}
	}

	assert_eq!(count, 60, "expected 60 messages");
	assert!(thumbnails.is_empty(), "expected all thumbnails to be deleted");
	assert!(segments.is_empty(), "expected all segments to be deleted");

	scuffle_utilsteardown(global, handler).await;
}

#[tokio::test]
async fn test_recording_boiler_plate() {
	let (global, handler, main_access_token) = scuffle_utilssetup(Default::default()).await;

	let no_scopes_token =
		scuffle_utilscreate_access_token(&global, &main_access_token.organization_id, vec![], HashMap::new()).await;

	let server = RecordingServer::<GlobalState>::new();

	use pb::scuffle::video::v1::recording_server::Recording as _;

	fn build_request<T>(global: &Arc<GlobalState>, token: &AccessToken, req: T) -> tonic::Request<T> {
		let mut req = tonic::Request::new(req);

		req.extensions_mut().insert(token.clone());
		req.extensions_mut().insert(global.clone());

		req
	}

	let s3_bucket = create_s3_bucket(&global, main_access_token.organization_id, HashMap::new()).await;
	let recording = create_recording(
		&global,
		main_access_token.organization_id,
		s3_bucket.id,
		None,
		None,
		HashMap::new(),
	)
	.await;

	let response = server
		.get(build_request(&global, &main_access_token, RecordingGetRequest::default()))
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
		.get(build_request(&global, &no_scopes_token, RecordingGetRequest::default()))
		.await
		.unwrap_err();
	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: recording:read");

	let response = server
		.modify(build_request(
			&global,
			&main_access_token,
			RecordingModifyRequest {
				id: Some(recording.id.into()),
				tags: Some(Tags {
					tags: vec![("test".to_string(), "test".to_string())].into_iter().collect(),
				}),
				..Default::default()
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
		.modify(build_request(&global, &no_scopes_token, RecordingModifyRequest::default()))
		.await
		.unwrap_err();
	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: recording:modify");

	let response = server
		.tag(build_request(
			&global,
			&main_access_token,
			RecordingTagRequest {
				id: Some(recording.id.into()),
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
		.tag(build_request(&global, &no_scopes_token, RecordingTagRequest::default()))
		.await
		.unwrap_err();
	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: recording:modify");

	let response = server
		.untag(build_request(
			&global,
			&main_access_token,
			RecordingUntagRequest {
				id: Some(recording.id.into()),
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
		.untag(build_request(&global, &no_scopes_token, RecordingUntagRequest::default()))
		.await
		.unwrap_err();
	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: recording:modify");

	let response = server
		.delete(build_request(
			&global,
			&main_access_token,
			RecordingDeleteRequest {
				ids: vec![Ulid::new().into()],
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
		.delete(build_request(&global, &no_scopes_token, RecordingDeleteRequest::default()))
		.await
		.unwrap_err();

	assert_eq!(response.code(), tonic::Code::PermissionDenied);
	assert_eq!(response.message(), "missing required scope: recording:delete");

	scuffle_utilsteardown(global, handler).await;
}
