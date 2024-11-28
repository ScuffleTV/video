use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use binary_helper::global::RequestGlobalExt;
use chrono::TimeZone;
use hyper::body::Incoming;
use hyper::{Request, Response, StatusCode};
use itertools::Itertools;
use pb::scuffle::video::internal::{LiveManifest, LiveRenditionManifest};
use pb::scuffle::video::v1::types::{AudioConfig, VideoConfig};
use prost::Message;
use scuffle_utils::database::non_null_vec;
use scuffle_utils::http::ext::*;
use scuffle_utils::http::router::builder::RouterBuilder;
use scuffle_utils::http::router::ext::RequestExt;
use scuffle_utils::http::router::Router;
use scuffle_utils::http::RouteError;
use scuffle_utils::prelude::FutureTimeout;
use scuffle_utilsmake_response;
use tokio::io::AsyncReadExt;
use tokio::time::Instant;
use ulid::Ulid;
use video_common::database::{Rendition, Room, RoomStatus, Visibility};
use video_common::keys;
use video_player_types::SessionRefresh;

use self::tokens::{ScreenshotClaims, SessionClaims, SessionClaimsType};
use super::error::Result;
use super::{Body, EdgeError};
use crate::edge::stream::hls_config::HlsConfig;
use crate::edge::stream::tokens::MediaClaims;
use crate::global::EdgeGlobal;

mod block_style;
mod hls_config;
mod playlist;
mod tokens;

fn organization_id(req: &Request<Incoming>) -> Result<Ulid> {
	Ulid::from_string(req.param("organization_id").unwrap())
		.map_err(|_| (StatusCode::BAD_REQUEST, "invalid organization_id").into())
}

fn room_id(req: &Request<Incoming>) -> Result<Ulid> {
	Ulid::from_string(req.param("room_id").unwrap()).map_err(|_| (StatusCode::BAD_REQUEST, "invalid room_id").into())
}

fn recording_id(req: &Request<Incoming>) -> Result<Ulid> {
	Ulid::from_string(req.param("recording_id").unwrap())
		.map_err(|_| (StatusCode::BAD_REQUEST, "invalid recording_id").into())
}

fn rendition(req: &Request<Incoming>) -> Result<Rendition> {
	req.param("rendition")
		.unwrap()
		.parse()
		.map_err(|_| (StatusCode::BAD_REQUEST, "invalid rendition").into())
}

fn token(req: &Request<Incoming>) -> Option<String> {
	req.uri().query().and_then(|v| {
		url::form_urlencoded::parse(v.as_bytes()).find_map(|(k, v)| if k == "token" { Some(v.to_string()) } else { None })
	})
}

async fn room_playlist<G: EdgeGlobal>(req: Request<Incoming>) -> Result<Response<Body>> {
	let config = HlsConfig::new(&req)?;

	let global = req.get_global::<G, _>()?;

	let organization_id = organization_id(&req)?;
	let room_id = room_id(&req)?;

	let client = global
		.db()
		.get()
		.await
		.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to get database"))?;

	let token = if let Some(token) = token(&req) {
		Some(tokens::TokenClaims::verify(&client, organization_id, tokens::TargetId::Room(room_id), &token).await?)
	} else {
		None
	};

	let room: Option<Room> = scuffle_utils::database::query(
		r#"
		SELECT
			*
		FROM
			rooms
		WHERE
			organization_id = $1
			AND id = $2
			AND status != $3
		"#,
	)
	.bind(organization_id)
	.bind(room_id)
	.bind(RoomStatus::Offline)
	.build_query_as()
	.fetch_optional(&client)
	.await
	.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to query database"))?;

	let room = room.ok_or((StatusCode::NOT_FOUND, "room not found"))?;

	let connection_id = room
		.active_ingest_connection_id
		.ok_or((StatusCode::NOT_FOUND, "room not found"))?;

	let audio_output = room.audio_output.ok_or((StatusCode::NOT_FOUND, "room not found"))?;

	let video_output = room.video_output.ok_or((StatusCode::NOT_FOUND, "room not found"))?;

	if room.visibility != Visibility::Public && token.is_none() {
		return Err((StatusCode::UNAUTHORIZED, "room is private, token is required").into());
	}

	let id = Ulid::new();
	let key_id = token
		.as_ref()
		.and_then(|t| t.header().key_id.as_ref())
		.map(|u| Ulid::from_string(u).unwrap());

	let global_config = global.config();
	let ip = if let Some(ip_header_mode) = &global_config.ip_header_mode {
		if let Some(ip) = req
			.headers()
			.get(ip_header_mode.to_lowercase().as_str())
			.and_then(|v| v.to_str().ok())
		{
			// https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/X-Forwarded-For#syntax
			let hops = ip.split(',').collect_vec();
			let client_idx = hops
				.len()
				.saturating_sub(global_config.ip_header_trusted_hops.unwrap_or(hops.len()));
			hops.get(client_idx).copied().and_then(|v| v.trim().parse().ok())
		} else {
			None
		}
	} else {
		req.data::<SocketAddr>().copied().map(|v| v.ip())
	}
	.ok_or((StatusCode::INTERNAL_SERVER_ERROR, "failed to get ip address"))?;

	scuffle_utils::database::query(
		r#"
		INSERT INTO playback_sessions (
			id,
			organization_id,
			room_id,
			user_id,
			playback_key_pair_id,
			issued_at,
			ip_address,
			user_agent,
			referer,
			origin,
			player_version
		) VALUES (
			$1,
			$2,
			$3,
			$4,
			$5,
			$6,
			$7,
			$8,
			$9,
			$10,
			$11
		)
		"#,
	)
	.bind(id)
	.bind(organization_id)
	.bind(room_id)
	.bind(token.as_ref().and_then(|t| t.claims().user_id.as_ref()))
	.bind(key_id)
	.bind(
		token
			.as_ref()
			.and_then(|t| chrono::Utc.timestamp_opt(t.claims().iat.unwrap(), 0).single()),
	)
	.bind(ip)
	.bind(req.headers().get("user-agent").map(|v| v.to_str().unwrap_or_default()))
	.bind(req.headers().get("referer").map(|v| v.to_str().unwrap_or_default()))
	.bind(req.headers().get("origin").map(|v| v.to_str().unwrap_or_default()))
	.bind(req.headers().get("x-player-version").map(|v| v.to_str().unwrap_or_default()))
	.build()
	.execute(client)
	.await
	.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to create session"))?;

	let manifest = playlist::room_playlist(
		&global,
		id,
		organization_id,
		connection_id,
		room_id,
		token.is_some(),
		&audio_output,
		&video_output,
	)?;

	let body = if config.scuffle_json {
		Body::from(
			serde_json::to_string(&manifest)
				.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to encode playlist"))?,
		)
	} else {
		Body::from(manifest.to_m3u8(organization_id))
	};

	let mut resp = Response::new(body);
	resp.headers_mut().insert(
		"Content-Type",
		if config.scuffle_json {
			"application/json"
		} else {
			"application/vnd.apple.mpegurl"
		}
		.parse()
		.unwrap(),
	);
	resp.headers_mut().insert("Cache-Control", "no-cache".parse().unwrap());

	Ok(resp)
}

#[derive(Debug, Clone, postgres_from_row::FromRow)]
struct RecordingExt {
	public: bool,
	#[from_row(from_fn = "non_null_vec")]
	renditions: Vec<Rendition>,
	#[from_row(from_fn = "non_null_vec")]
	configs: Vec<Vec<u8>>,
}

async fn recording_playlist<G: EdgeGlobal>(req: Request<Incoming>) -> Result<Response<Body>> {
	let config = HlsConfig::new(&req)?;

	let global = req.get_global::<G, _>()?;

	let organization_id = organization_id(&req)?;
	let recording_id = recording_id(&req)?;

	let client = global
		.db()
		.get()
		.await
		.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to get database"))?;

	let token = if let Some(token) = token(&req) {
		Some(tokens::TokenClaims::verify(&client, organization_id, tokens::TargetId::Recording(recording_id), &token).await?)
	} else {
		None
	};

	let recording: Option<RecordingExt> = scuffle_utils::database::query(
		r#"
        WITH filtered_recordings AS (
            SELECT
                id,
                public
            FROM recordings 
            WHERE 
                id = $1
                AND organization_id = $2
                AND deleted = FALSE
        )

        SELECT 
            r.public as public,
            ARRAY_AGG(rr.rendition) as renditions,
            ARRAY_AGG(rr.config) as configs
        FROM 
            filtered_recordings AS r
        INNER JOIN recording_renditions rr
            ON r.id = rr.recording_id 
        GROUP BY 
            r.public
    	"#,
	)
	.bind(recording_id)
	.bind(organization_id)
	.build_query_as()
	.fetch_optional(&client)
	.await
	.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to query database"))?;

	let recording = recording.ok_or((StatusCode::NOT_FOUND, "recording not found"))?;

	if !recording.public && token.is_none() {
		return Err((StatusCode::UNAUTHORIZED, "recording is private, token is required").into());
	}

	let audio_output = recording
		.renditions
		.iter()
		.zip(recording.configs.iter())
		.filter_map(|(r, c)| {
			if r.is_audio() {
				Some(AudioConfig::decode(c.as_slice()))
			} else {
				None
			}
		})
		.collect::<Result<Vec<_>, _>>()
		.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to decode audio configs"))?;

	let video_output = recording
		.renditions
		.iter()
		.zip(recording.configs.iter())
		.filter_map(|(r, c)| {
			if r.is_video() {
				Some(VideoConfig::decode(c.as_slice()))
			} else {
				None
			}
		})
		.collect::<Result<Vec<_>, _>>()
		.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to decode video configs"))?;

	let id = Ulid::new();

	let global_config = global.config();
	let ip = if let Some(ip_header_mode) = &global_config.ip_header_mode {
		if let Some(ip) = req
			.headers()
			.get(ip_header_mode.to_lowercase().as_str())
			.and_then(|v| v.to_str().ok())
		{
			let hops = ip.split(',').collect_vec();
			hops.get(global_config.ip_header_trusted_hops.unwrap_or(hops.len() - 1).max(hops.len()))
				.copied()
				.and_then(|v| v.trim().parse().ok())
		} else {
			None
		}
	} else {
		req.data::<SocketAddr>().copied().map(|v| v.ip())
	}
	.ok_or((StatusCode::INTERNAL_SERVER_ERROR, "failed to get ip address"))?;

	scuffle_utils::database::query(
		r#"
		INSERT INTO playback_sessions (
			id,
			organization_id,
			recording_id,
			user_id,
			playback_key_pair_id,
			issued_at,
			ip_address,
			user_agent,
			referer,
			origin,
			player_version
		) VALUES (
			$1,
			$2,
			$3,
			$4,
			$5,
			$6,
			$7,
			$8,
			$9,
			$10,
			$11
		)
		"#,
	)
	.bind(id)
	.bind(organization_id)
	.bind(recording_id)
	.bind(token.as_ref().and_then(|t| t.claims().user_id.as_ref()))
	.bind(token.as_ref().and_then(|t| t.header().key_id.as_ref()))
	.bind(
		token
			.as_ref()
			.and_then(|t| chrono::Utc.timestamp_opt(t.claims().iat.unwrap(), 0).single()),
	)
	.bind(ip)
	.bind(req.headers().get("user-agent").map(|v| v.to_str().unwrap_or_default()))
	.bind(req.headers().get("referer").map(|v| v.to_str().unwrap_or_default()))
	.bind(req.headers().get("origin").map(|v| v.to_str().unwrap_or_default()))
	.bind(req.headers().get("x-player-version").map(|v| v.to_str().unwrap_or_default()))
	.build()
	.execute(client)
	.await
	.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to create session"))?;

	let manifest = playlist::recording_playlist(
		&global,
		id,
		organization_id,
		recording_id,
		token.is_some(),
		&audio_output,
		&video_output,
	)?;

	let body = if config.scuffle_json {
		Body::from(
			serde_json::to_string(&manifest)
				.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to encode playlist"))?,
		)
	} else {
		Body::from(manifest.to_m3u8(organization_id))
	};

	let mut resp = Response::new(body);
	resp.headers_mut().insert(
		"Content-Type",
		if config.scuffle_json {
			"application/json"
		} else {
			"application/vnd.apple.mpegurl"
		}
		.parse()
		.unwrap(),
	);
	resp.headers_mut().insert("Cache-Control", "no-cache".parse().unwrap());

	Ok(resp)
}

async fn session_playlist<G: EdgeGlobal>(req: Request<Incoming>) -> Result<Response<Body>> {
	let config = HlsConfig::new(&req)?;
	let global = req.get_global::<G, _>()?;

	let organization_id = organization_id(&req)?;

	let session = req.param("session").unwrap();

	let rendition = rendition(&req)?;

	let session = SessionClaims::verify(&global, organization_id, session)?;

	let client = global
		.db()
		.get()
		.await
		.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to get database"))?;

	let resp = scuffle_utils::database::query(
		r#"
		UPDATE playback_sessions SET
			expires_at = NOW() + INTERVAL '10 minutes'
		WHERE
			id = $1 AND
			organization_id = $2 AND
			expires_at > NOW()
		"#,
	)
	.bind(session.id)
	.bind(session.organization_id)
	.build()
	.execute(&client)
	.timeout(Duration::from_secs(2))
	.await
	.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to update session: timedout"))?
	.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to update session"))?;

	if resp == 0 {
		return Err((StatusCode::BAD_REQUEST, "invalid session, expired or not found").into());
	}

	let manifest = if let SessionClaimsType::Room { room_id, connection_id } = session.ty {
		let mut subscription = global
			.subscriber()
			.subscribe_kv(keys::rendition_manifest(organization_id, room_id, connection_id, rendition))
			.timeout(Duration::from_secs(2))
			.await
			.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to get manifest: timedout"))?
			.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to get manifest"))?;

		let mut manifest: LiveRenditionManifest;

		let now = Instant::now();

		loop {
			let result = subscription
				.next()
				.timeout(Duration::from_secs(2))
				.await
				.map_err_route((StatusCode::BAD_REQUEST, "manifest watch time timedout"))?
				.ok_or((StatusCode::INTERNAL_SERVER_ERROR, "manifest watch returned invalid value"))?;

			manifest = LiveRenditionManifest::decode(result.value)
				.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "failed to decode manifest"))?;

			let info = manifest
				.info
				.as_ref()
				.ok_or((StatusCode::INTERNAL_SERVER_ERROR, "manifest missing info"))?;

			if manifest.completed || !config.is_blocked(info) {
				break;
			}

			if now.elapsed() > Duration::from_secs(5) {
				return Err((StatusCode::BAD_REQUEST, "segment watch time timedout").into());
			}
		}

		Some(manifest)
	} else {
		None
	};

	let playlist = playlist::rendition_playlist(&global, &client, &session, &config, rendition, manifest.as_ref()).await?;
	let body = if config.scuffle_json {
		Body::from(
			serde_json::to_string(&playlist)
				.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to encode playlist"))?,
		)
	} else {
		Body::from(playlist.to_m3u8(
			organization_id,
			match session.ty {
				SessionClaimsType::Room { room_id, .. } => Some(room_id),
				_ => None,
			},
		))
	};

	let mut resp = Response::new(body);
	resp.headers_mut().insert(
		"Content-Type",
		if config.scuffle_json {
			"application/json"
		} else {
			"application/vnd.apple.mpegurl"
		}
		.parse()
		.unwrap(),
	);
	resp.headers_mut().insert("Cache-Control", "no-cache".parse().unwrap());

	Ok(resp)
}

async fn session_refresh<G: EdgeGlobal>(req: Request<Incoming>) -> Result<Response<Body>> {
	let global = req.get_global::<G, _>()?;

	let organization_id = organization_id(&req)?;

	let session = req.param("session").unwrap();

	let session = SessionClaims::verify(&global, organization_id, session)?;

	let resp = scuffle_utils::database::query(
		r#"
		UPDATE playback_sessions SET
			expires_at = NOW() + INTERVAL '10 minutes'
		WHERE
			id = $1 AND
			organization_id = $2 AND
			expires_at > NOW()
		"#,
	)
	.bind(session.id)
	.bind(session.organization_id)
	.build()
	.execute(global.db())
	.await
	.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to update session"))?;

	if resp == 0 {
		return Err((StatusCode::BAD_REQUEST, "invalid session, expired or not found").into());
	}

	let mut resp = Response::new(Body::from(serde_json::to_vec(&SessionRefresh { success: true }).unwrap()));
	resp.headers_mut().insert("Content-Type", "application/json".parse().unwrap());
	resp.headers_mut().insert("Cache-Control", "no-cache".parse().unwrap());

	Ok(resp)
}

async fn room_media<G: EdgeGlobal>(req: Request<Incoming>) -> Result<Response<Body>> {
	let global = req.get_global::<G, _>()?;

	let organization_id = organization_id(&req)?;

	let room_id = room_id(&req)?;

	let media = req.param("media").unwrap();

	let claims = MediaClaims::verify(&global, organization_id, room_id, media)?;

	let mut subscriber = global
		.subscriber()
		.subscribe_kv(keys::rendition_manifest(
			organization_id,
			room_id,
			claims.connection_id,
			claims.rendition,
		))
		.await
		.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to get manifest"))?;

	let keys: Vec<String>;
	let now = Instant::now();
	loop {
		if now.elapsed() > Duration::from_secs(5) {
			return Err((StatusCode::BAD_REQUEST, "media watch time timedout").into());
		}

		let playlist = subscriber
			.next()
			.await
			.ok_or((StatusCode::INTERNAL_SERVER_ERROR, "manifest watch returned invalid value"))?;
		let playlist = LiveRenditionManifest::decode(playlist.value)
			.map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "failed to decode manifest"))?;
		let info = playlist
			.info
			.as_ref()
			.ok_or((StatusCode::INTERNAL_SERVER_ERROR, "manifest missing info"))?;

		keys = match claims.ty {
			tokens::MediaClaimsType::Init => {
				vec![keys::init(organization_id, room_id, claims.connection_id, claims.rendition)]
			}
			tokens::MediaClaimsType::Part(idx) => {
				if info.next_part_idx <= idx {
					if playlist.completed {
						return Err(RouteError::from(make_response!(
							StatusCode::NOT_FOUND,
							serde_json::json!({
								"message": "part not found",
								"finished": true,
								"success": false,
							})
						)));
					}

					continue;
				}

				if playlist
					.segments
					.first()
					.and_then(|s| s.parts.first())
					.map(|p| p.idx)
					.unwrap_or(0) > idx
				{
					return Err((StatusCode::NOT_FOUND, "part not found").into());
				}

				vec![keys::part(
					organization_id,
					room_id,
					claims.connection_id,
					claims.rendition,
					idx,
				)]
			}
			tokens::MediaClaimsType::Segment(idx) => {
				// The segment is still being written, so we return not found, this should not
				// be possible.
				if info.next_segment_idx == idx + 1 {
					tracing::warn!(idx, "segment not found");
					return Err((StatusCode::NOT_FOUND, "segment not found").into());
				}

				let Some(segment) = playlist.segments.iter().find(|s| s.idx == idx) else {
					return Err((StatusCode::NOT_FOUND, "segment not found").into());
				};

				segment
					.parts
					.iter()
					.map(|p| keys::part(organization_id, room_id, claims.connection_id, claims.rendition, p.idx))
					.collect()
			}
		};

		break;
	}

	drop(subscriber);

	// Streaming response
	let mut data = Vec::new();

	for key in &keys {
		let mut item = global
			.media_store()
			.get(key)
			.await
			.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to get media"))?;

		item.read_to_end(&mut data)
			.timeout(Duration::from_secs(2))
			.await
			.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to read media, timedout"))?
			.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to read media, unknown"))?;
	}

	let mut resp = Response::new(Body::from(data));
	resp.headers_mut().insert("Content-Type", "video/mp4".parse().unwrap());
	resp.headers_mut()
		.insert("Cache-Control", "max-age=31536000".parse().unwrap());

	Ok(resp)
}

async fn room_screenshot<G: EdgeGlobal>(req: Request<Incoming>) -> Result<Response<Body>> {
	let global = req.get_global::<G, _>()?;

	let organization_id = organization_id(&req)?;
	let room_id = room_id(&req)?;

	let client = global
		.db()
		.get()
		.await
		.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to get database"))?;

	let token = if let Some(token) = token(&req) {
		Some(tokens::TokenClaims::verify(&client, organization_id, tokens::TargetId::Room(room_id), &token).await?)
	} else {
		None
	};

	let room: Option<Room> = scuffle_utils::database::query(
		r#"
		SELECT
			*
		FROM
			rooms
		WHERE
			organization_id = $1
			AND id = $2
			AND status != $3
		"#,
	)
	.bind(organization_id)
	.bind(room_id)
	.bind(RoomStatus::Offline)
	.build_query_as()
	.fetch_optional(client)
	.await
	.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to query database"))?;

	let room = room.ok_or((StatusCode::NOT_FOUND, "room not found"))?;

	let connection_id = room
		.active_ingest_connection_id
		.ok_or((StatusCode::NOT_FOUND, "room not found"))?;

	if room.visibility != Visibility::Public && token.is_none() {
		return Err((StatusCode::UNAUTHORIZED, "room is private, token is required").into());
	}

	// We have permission to see the screenshot.
	let manifest = global
		.metadata_store()
		.get(keys::manifest(organization_id, room_id, connection_id))
		.await
		.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to get manifest"))?
		.ok_or((StatusCode::NOT_FOUND, "manifest not found"))?;

	let manifest =
		LiveManifest::decode(manifest).map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to decode manifest"))?;

	let screenshot = ScreenshotClaims {
		connection_id,
		idx: manifest.screenshot_idx,
		organization_id,
		room_id,
	}
	.sign(&global)?;

	let mut response = Response::new(Body::default());

	*response.status_mut() = StatusCode::TEMPORARY_REDIRECT;

	let url = format!("/{organization_id}/{room_id}/{screenshot}.jpg");

	response.headers_mut().insert("Location", url.parse().unwrap());

	response.headers_mut().insert("Cache-Control", "no-cache".parse().unwrap());

	Ok(response)
}

async fn room_screenshot_media<G: EdgeGlobal>(req: Request<Incoming>) -> Result<Response<Body>> {
	let global = req.get_global::<G, _>()?;

	let organization_id = Ulid::from_string(req.param("organization_id").unwrap())
		.map_err(|_| (StatusCode::BAD_REQUEST, "invalid organization_id"))?;

	let room_id =
		Ulid::from_string(req.param("room_id").unwrap()).map_err(|_| (StatusCode::BAD_REQUEST, "invalid room_id"))?;

	let screenshot = req.param("screenshot").unwrap();

	let claims = ScreenshotClaims::verify(&global, organization_id, room_id, screenshot)?;

	if claims.organization_id != organization_id {
		return Err((StatusCode::BAD_REQUEST, "invalid media, organization_id mismatch").into());
	}

	if claims.room_id != room_id {
		return Err((StatusCode::BAD_REQUEST, "invalid media, room_name mismatch").into());
	}

	let key = keys::screenshot(organization_id, room_id, claims.connection_id, claims.idx);

	tracing::debug!(key = %key, "getting screenshot");

	let mut item = global
		.media_store()
		.get(&key)
		.await
		.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to get screenshot"))?;

	let mut buf = Vec::new();

	item.read_to_end(&mut buf)
		.await
		.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to read screenshot"))?;

	let mut resp = Response::new(Body::from(buf));
	resp.headers_mut().insert("Content-Type", "image/jpeg".parse().unwrap());
	resp.headers_mut()
		.insert("Cache-Control", "max-age=31536000".parse().unwrap());

	Ok(resp)
}

pub fn routes<G: EdgeGlobal>(_: &Arc<G>) -> RouterBuilder<Incoming, Body, RouteError<EdgeError>> {
	Router::builder()
		.get("/:organization_id/:room_id.m3u8", room_playlist::<G>)
		.get("/:organization_id/r/:recording_id.m3u8", recording_playlist::<G>)
		.get("/:organization_id/:session/:rendition.m3u8", session_playlist::<G>)
		.get("/:organization_id/:session/refresh", session_refresh::<G>)
		.get("/:organization_id/:room_id.jpg", room_screenshot::<G>)
		.get("/:organization_id/:room_id/:media.mp4", room_media::<G>)
		.get("/:organization_id/:room_id/:screenshot.jpg", room_screenshot_media::<G>)
}
