use std::sync::Arc;

use hyper::StatusCode;
use pb::ext::UlidExt;
use pb::scuffle::video::internal::LiveRenditionManifest;
use pb::scuffle::video::v1::types::{AudioConfig, VideoConfig};
use scuffle_utils::database::non_null_vec;
use scuffle_utils::http::ext::*;
use ulid::Ulid;
use video_common::database::{Recording, RecordingThumbnail, Rendition, Visibility};
use video_player_types::{
	RenditionPlaylist, RenditionPlaylistRendition, RenditionPlaylistSegment, RenditionPlaylistSegmentPart,
	RoomPlaylistTrack, RoomPlaylistTrackAudio, RoomPlaylistTrackVideo, SessionPlaylist, ThumbnailRange,
};

use super::hls_config::HlsConfig;
use super::tokens::{MediaClaimsType, SessionClaims, SessionClaimsType};
use crate::edge::error::Result;
use crate::edge::stream::tokens::MediaClaims;
use crate::global::EdgeGlobal;

#[derive(Debug, Clone, postgres_from_row::FromRow)]
struct RecordingExt {
	pub public_url: String,
	#[from_row(flatten)]
	pub recording: Recording,
}

#[derive(Debug, Clone, postgres_from_row::FromRow)]
struct RecordingRenditionExt {
	#[from_row(from_fn = "non_null_vec")]
	pub segment_ids: Vec<Ulid>,
	#[from_row(from_fn = "non_null_vec")]
	pub segment_indexes: Vec<i32>,
	#[from_row(from_fn = "non_null_vec")]
	pub segment_start_times: Vec<f32>,
	#[from_row(from_fn = "non_null_vec")]
	pub segment_end_times: Vec<f32>,
}

#[inline(always)]
fn normalize_float(f: f64) -> f64 {
	(f * 1000.0).round() / 1000.0
}

#[allow(clippy::too_many_arguments)]
pub fn room_playlist<G: EdgeGlobal>(
	global: &Arc<G>,
	id: Ulid,
	organization_id: Ulid,
	connection_id: Ulid,
	room_id: Ulid,
	was_authenticated: bool,
	audio_output: &[AudioConfig],
	video_output: &[VideoConfig],
) -> Result<SessionPlaylist> {
	let session = SessionClaims {
		id,
		organization_id,
		ty: SessionClaimsType::Room { connection_id, room_id },
		was_authenticated,
		iat: chrono::Utc::now().timestamp(),
	}
	.sign(global)?;

	Ok(SessionPlaylist {
		audio_tracks: audio_output
			.iter()
			.map(|a| RoomPlaylistTrack {
				name: Rendition::from(a.rendition()).to_string(),
				bitrate: a.bitrate as u32,
				codec: a.codec.clone(),
				other: RoomPlaylistTrackAudio {
					channels: a.channels as u32,
					sample_rate: a.sample_rate as u32,
				},
			})
			.collect(),
		video_tracks: video_output
			.iter()
			.map(|v| RoomPlaylistTrack {
				name: Rendition::from(v.rendition()).to_string(),
				bitrate: v.bitrate as u32,
				codec: v.codec.clone(),
				other: RoomPlaylistTrackVideo {
					frame_rate: v.fps as u32,
					width: v.width as u32,
					height: v.height as u32,
				},
			})
			.collect(),
		session,
	})
}

#[allow(clippy::too_many_arguments)]
pub fn recording_playlist<G: EdgeGlobal>(
	global: &Arc<G>,
	id: Ulid,
	organization_id: Ulid,
	recording_id: Ulid,
	was_authenticated: bool,
	audio_output: &[AudioConfig],
	video_output: &[VideoConfig],
) -> Result<SessionPlaylist> {
	let session = SessionClaims {
		id,
		organization_id,
		ty: SessionClaimsType::Recording { recording_id },
		was_authenticated,
		iat: chrono::Utc::now().timestamp(),
	}
	.sign(global)?;

	Ok(SessionPlaylist {
		audio_tracks: audio_output
			.iter()
			.map(|a| RoomPlaylistTrack {
				name: Rendition::from(a.rendition()).to_string(),
				bitrate: a.bitrate as u32,
				codec: a.codec.clone(),
				other: RoomPlaylistTrackAudio {
					channels: a.channels as u32,
					sample_rate: a.sample_rate as u32,
				},
			})
			.collect(),
		video_tracks: video_output
			.iter()
			.map(|v| RoomPlaylistTrack {
				name: Rendition::from(v.rendition()).to_string(),
				bitrate: v.bitrate as u32,
				codec: v.codec.clone(),
				other: RoomPlaylistTrackVideo {
					frame_rate: v.fps as u32,
					width: v.width as u32,
					height: v.height as u32,
				},
			})
			.collect(),
		session,
	})
}

pub async fn rendition_playlist<G: EdgeGlobal>(
	global: &Arc<G>,
	client: &utils::database::tokio_postgres::Client,
	session: &SessionClaims,
	config: &HlsConfig,
	rendition: Rendition,
	manifest: Option<&LiveRenditionManifest>,
) -> Result<RenditionPlaylist> {
	let organization_id = session.organization_id;

	let mut playlist = RenditionPlaylist::default();

	let recording_data = match (manifest, session.ty) {
		(Some(_), SessionClaimsType::Recording { .. }) => {
			return Err((StatusCode::INTERNAL_SERVER_ERROR, "recording session with manifest").into());
		}
		(None, SessionClaimsType::Room { .. }) => {
			return Err((StatusCode::INTERNAL_SERVER_ERROR, "room session without manifest").into());
		}
		(None, SessionClaimsType::Recording { recording_id }) => {
			playlist.init_segment_id = "init.mp4".to_string();
			playlist.init_dvr = true;
			Some((recording_id, false, None))
		}
		(Some(manifest), SessionClaimsType::Room { connection_id, room_id }) => {
			playlist.msn = manifest.segments.first().map(|s| s.idx).unwrap_or_default();
			playlist.init_segment_id = MediaClaims {
				connection_id,
				organization_id: session.organization_id,
				rendition,
				room_id,
				ty: MediaClaimsType::Init,
			}
			.sign(global)?;

			if config.scuffle_dvr {
				manifest.recording_data.as_ref().map(|d| {
					(
						d.recording_ulid.into_ulid(),
						config.skip,
						manifest.segments.first().map(|s| s.idx),
					)
				})
			} else {
				None
			}
		}
	};

	let recording_data = if let Some((recording_id, skip, active_idx)) = recording_data {
		scuffle_utils::database::query(
			r#"
            SELECT
                s.public_url,
                r.*
            FROM recordings r
            INNER JOIN s3_buckets s
                ON s.id = r.s3_bucket_id
            WHERE
                r.id = $1
                AND r.organization_id = $2
                AND r.deleted = FALSE
                AND r.allow_dvr = TRUE
        	"#,
		)
		.bind(recording_id)
		.bind(organization_id)
		.build_query_as()
		.fetch_optional(client)
		.await
		.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to query database"))?
		.and_then(|r: RecordingExt| {
			if r.recording.visibility == Visibility::Public || session.was_authenticated {
				Some((recording_id, skip, active_idx, r.public_url))
			} else {
				None
			}
		})
	} else {
		None
	};

	if let Some((recording_id, skip, active_idx, public_url)) = &recording_data {
		playlist.msn = 0;

		playlist.dvr_prefix = Some(
			format!("{public_url}/{organization_id}/{recording_id}/{rendition}")
				.parse()
				.unwrap(),
		);
		playlist.thumbnail_prefix = Some(
			format!("{public_url}/{organization_id}/{recording_id}/thumbnails")
				.parse()
				.unwrap(),
		);

		if !*skip {
			let recording_rendition: RecordingRenditionExt = scuffle_utils::database::query(
				r#"
                WITH filtered_renditions AS (
                    SELECT recording_id, rendition
                    FROM recording_renditions 
                    WHERE recording_id = $1 AND rendition = $2
                )

                SELECT
                    r.recording_id,
                    r.rendition,
                    ARRAY_AGG(rs.id) as segment_ids,
                    ARRAY_AGG(rs.idx) as segment_indexes,
                    ARRAY_AGG(rs.start_time) as segment_start_times,
                    ARRAY_AGG(rs.end_time) as segment_end_times
                FROM filtered_renditions AS r
                LEFT JOIN recording_rendition_segments as rs
                    ON rs.rendition = r.rendition
                    AND rs.recording_id = r.recording_id
                GROUP BY
                    r.recording_id,
                    r.rendition;
                "#,
			)
			.bind(recording_id)
			.bind(rendition)
			.build_query_as()
			.fetch_optional(client)
			.await
			.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to query database"))?
			.ok_or((StatusCode::NOT_FOUND, "recording no longer exists"))?;

			let recording_thumbnails: Vec<RecordingThumbnail> = scuffle_utils::database::query(
				r#"
                SELECT
                    *
                FROM recording_thumbnails
                WHERE 
                    recording_id = $1
                "#,
			)
			.bind(recording_id)
			.build_query_as()
			.fetch_all(client)
			.await
			.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to query database"))?;

			playlist.thumbnails = recording_thumbnails
				.into_iter()
				.map(|t| ThumbnailRange {
					id: format!("{}.{}.jpg", t.idx, t.id),
					start_time: normalize_float(t.start_time as f64),
					idx: t.idx as u32,
				})
				.collect();

			let mut discontinuity_count = 0;

			for (true_idx, (segment_idx, start_time, end_time, segment_id)) in recording_rendition
				.segment_indexes
				.iter()
				.copied()
				.zip(recording_rendition.segment_start_times.iter().copied())
				.zip(recording_rendition.segment_end_times.iter().copied())
				.zip(recording_rendition.segment_ids.iter().copied())
				.map(|(((idx, start_time), end_time), id)| (idx as u32, start_time, end_time, id))
				.take_while(|(idx, _, _, _)| active_idx.map(|aidx| *idx < aidx).unwrap_or(true))
				.enumerate()
			{
				if true_idx + discontinuity_count != segment_idx as usize {
					playlist.segments.push(RenditionPlaylistSegment {
						start_time: None,
						end_time: None,
						dvr_tag: None,
						id: None,
						idx: segment_idx,
						parts: vec![],
					});
					discontinuity_count += 1;
					continue;
				};

				let dvr_tag = format!("{segment_idx}.{segment_id}.mp4");

				let end_time = normalize_float(end_time as f64);
				let start_time = normalize_float(start_time as f64);

				playlist.segments.push(RenditionPlaylistSegment {
					end_time: Some(end_time),
					start_time: Some(start_time),
					dvr_tag: Some(dvr_tag),
					id: None,
					idx: segment_idx,
					parts: vec![],
				});
			}
		} else if let Some(manifest) = manifest {
			playlist.thumbnails = manifest
				.recording_data
				.as_ref()
				.map(|d| {
					d.thumbnails
						.iter()
						.map(|t| ThumbnailRange {
							id: format!("{}.{}.jpg", t.idx, t.ulid.into_ulid()),
							start_time: normalize_float(t.timestamp as f64),
							idx: t.idx,
						})
						.collect()
				})
				.unwrap_or_default();
		}
	}

	if let Some(manifest) = manifest {
		let mut current_duration = manifest.total_duration
			- manifest
				.segments
				.iter()
				.flat_map(|s| s.parts.iter().map(|p| p.duration as u64))
				.sum::<u64>();

		let info = manifest
			.info
			.as_ref()
			.ok_or((StatusCode::INTERNAL_SERVER_ERROR, "missing rendition info"))?;

		let (connection_id, room_id) = match session.ty {
			SessionClaimsType::Room { room_id, connection_id } => (connection_id, room_id),
			_ => unreachable!(),
		};

		for segment in &manifest.segments {
			let mut parts = Vec::new();
			if segment.idx >= info.next_segment_idx.saturating_sub(2) {
				for part in &segment.parts {
					let part_jwt = MediaClaims {
						connection_id,
						ty: MediaClaimsType::Part(part.idx),
						organization_id: session.organization_id,
						rendition,
						room_id,
					}
					.sign(global)?;

					let duration = part.duration as f64 / manifest.timescale as f64;
					parts.push(RenditionPlaylistSegmentPart {
						id: part_jwt,
						duration,
						independent: part.independent,
					})
				}
			}

			let start_time = current_duration as f64 / manifest.timescale as f64;

			let (segment_id, end_time, dvr_tag) =
				if segment.idx != info.next_segment_idx.saturating_sub(1) || manifest.completed {
					let segment_jwt = MediaClaims {
						connection_id,
						ty: MediaClaimsType::Segment(segment.idx),
						organization_id: session.organization_id,
						rendition,
						room_id,
					}
					.sign(global)?;

					current_duration += segment.parts.iter().map(|p| p.duration as u64).sum::<u64>();

					let end_time = current_duration as f64 / manifest.timescale as f64;

					let dvr_tag = if recording_data.is_some() {
						let id = segment.id.into_ulid();
						let idx = segment.idx;
						Some(format!("{idx}.{id}.mp4"))
					} else {
						None
					};

					(Some(segment_jwt), Some(end_time), dvr_tag)
				} else {
					(None, None, None)
				};

			playlist.segments.push(RenditionPlaylistSegment {
				id: segment_id,
				start_time: Some(start_time),
				end_time,
				dvr_tag,
				parts,
				idx: segment.idx,
			});
		}

		playlist.finished = manifest.completed;
		if !manifest.completed {
			for i in 0..16 {
				let part_idx = info.next_part_idx + i;

				let part_jwt = MediaClaims {
					connection_id,
					ty: MediaClaimsType::Part(part_idx),
					organization_id,
					rendition,
					room_id,
				}
				.sign(global)?;

				playlist.pre_fetch_part_ids.push(part_jwt);
				playlist.last_pre_fetch_part_idx = part_idx;
			}

			for (rendition, info) in manifest.other_info.iter() {
				let last_msn = info.next_segment_idx.saturating_sub(1);
				let last_part = info.next_segment_part_idx.saturating_sub(1);

				playlist.renditions.push(RenditionPlaylistRendition {
					name: rendition.clone(),
					last_segment_idx: last_msn,
					last_segment_part_idx: last_part,
					last_independent_part_idx: info.last_independent_part_idx,
				});
			}
		} else {
			playlist.last_pre_fetch_part_idx = info.next_part_idx.saturating_sub(1);
		}
	} else {
		playlist.finished = true;
	}

	Ok(playlist)
}
