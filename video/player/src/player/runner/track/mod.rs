use bytes::Bytes;
use url::Url;
use video_player_types::{RenditionPlaylist, RenditionPlaylistRendition, RoomPlaylistTrack};

use crate::player::api::{ApiMediaClient, ApiSessionClient, Json, RenditionSettings, ScufflePart};
use crate::player::errors::{ErrorCode, EventError, EventErrorExtFetch};
use crate::player::fetch::{FetchError, FetchRequest};
use crate::player::inner::PlayerInnerHolder;
use crate::player::util::now;
use crate::player::PlayerResult;

mod regions;
mod requests;

use self::regions::{SegmentRangeResult, SegmentRegions, TimeRegion, TimeRegions};
use self::requests::{RequestQueue, TrackRequest};

const MAX_ERROR_COUNT: u32 = 10;
const ERROR_BACKOFF: f64 = 250.0;

#[derive(Debug)]
struct ManifestRequest {
	req: Json<RenditionPlaylist>,
	requested_at: f64,
	was_dvr: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ManifestRefresh {
	Time(f64),
	Part(u32),
	IPart(u32),
	Never,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum StartFrom {
	IPart(u32),
	Time(f64),
}

#[derive(Debug)]
pub struct TrackState<T> {
	track: RoomPlaylistTrack<T>,

	client: ApiSessionClient,
	media_client: Option<ApiMediaClient>,

	init_segment: Option<Bytes>,

	init_request: Option<TrackRequest>,
	manifest_request: Option<ManifestRequest>,
	requests: RequestQueue,

	requested_regions: TimeRegions,
	segment_regions: SegmentRegions,

	manifest: Option<(RenditionPlaylist, f64)>,

	manifest_refresh: ManifestRefresh,

	player_time: f64,
	dvr_prefix: Option<Url>,
	last_requested_part_idx: Option<u32>,
	last_requested_segment_idx: Option<u32>,

	start_from: Option<StartFrom>,

	will_start_at: Option<f64>,

	manifest_req_error_count: u32,
	init_req_error_count: u32,

	next_init_req_time: f64,

	timescale: u32,

	finished: bool,

	stopped: bool,

	last_session_refreshed: f64,
}

#[derive(Debug)]
pub enum TrackResult {
	/// New media has been loaded,
	/// The first value is the bytes of the media, the second value is the start
	/// time of the media, the third value is the end time of the media
	Media {
		data: Bytes,
		start_time: f64,
		end_time: f64,
		decode_time: u64,
		duration: u32,
	},
	/// A discontinuity has occurred, the first value is the end time of the
	/// previous segment, the second value is the start time of the next segment
	/// Ie. the time that was requested is not available
	Discontinuity(f64, f64),
}

impl<T> TrackState<T> {
	pub fn new(client: ApiSessionClient, media_client: Option<ApiMediaClient>, track: RoomPlaylistTrack<T>) -> Self {
		Self {
			track,
			client,
			media_client,
			init_segment: None,
			manifest_request: None,
			requested_regions: TimeRegions::new(),
			segment_regions: SegmentRegions::new(),
			player_time: 0.0,
			init_request: None,
			manifest_refresh: ManifestRefresh::Time(now()),
			last_requested_part_idx: None,
			last_requested_segment_idx: None,
			requests: RequestQueue::new(),
			manifest: None,
			dvr_prefix: None,
			start_from: None,
			timescale: 1,
			will_start_at: None,
			manifest_req_error_count: 0,
			init_req_error_count: 0,
			next_init_req_time: -1.0,
			stopped: true,
			finished: false,
			last_session_refreshed: -1.0,
		}
	}

	pub fn timescle(&self) -> u32 {
		self.timescale
	}

	pub fn last_session_refreshed(&self) -> f64 {
		self.last_session_refreshed
	}

	pub fn remove_regions(&mut self, start: f64, end: f64) {
		self.requested_regions.remove(start, end);
	}

	pub fn track(&self) -> &RoomPlaylistTrack<T> {
		&self.track
	}

	pub fn finished(&self) -> bool {
		self.finished
	}

	pub fn init_segment(&self) -> Option<&Bytes> {
		self.init_segment.as_ref()
	}

	pub fn will_start_at(&self) -> Option<f64> {
		self.will_start_at
	}

	pub fn seekable_range(&self) -> (f64, f64) {
		let start_time = if self.segment_regions.is_empty() {
			self.manifest
				.as_ref()
				.and_then(|(m, _)| m.segments.iter().find(|s| s.start_time.is_some()))
				.and_then(|s| s.start_time)
				.unwrap_or(0.0)
		} else {
			0.0
		};

		let end_time = self
			.manifest
			.as_ref()
			.and_then(|(m, _)| m.segments.last())
			.map(|s| s.start_time.unwrap() + s.duration())
			.unwrap_or(0.0);

		(start_time, end_time)
	}

	pub fn stop(&mut self) {
		if !self.stopped {
			tracing::debug!(name = self.track.name, "stopping track");
		}

		self.stopped = true;
		self.manifest_request = None;
		self.requests.clear();
		self.last_requested_part_idx = None;
		self.last_requested_segment_idx = None;
		self.requested_regions.clear();
		self.init_request = None;
		if !self.finished {
			self.manifest_refresh = ManifestRefresh::Time(now());
		}
	}

	pub fn start(&mut self) {
		tracing::debug!(name = self.track.name, "started track");
		self.will_start_at = None;

		if self.start_from.is_some() {
			self.start_from = None;
			if !self.finished {
				self.manifest_refresh = ManifestRefresh::Time(now());
			}
		}
	}

	pub fn start_from_time(&mut self, time: f64) {
		tracing::debug!(name = self.track.name, "starting track from time: {time}");
		self.start_from = Some(StartFrom::Time(time));
	}

	pub fn start_at_ipart(&mut self, idx: u32) {
		tracing::debug!(name = self.track.name, "starting track at ipart: {idx}");
		self.start_from = Some(StartFrom::IPart(idx));

		if !self.finished {
			self.manifest_refresh = ManifestRefresh::IPart(idx);
		}
	}

	pub fn start_at_part(&mut self, idx: u32) {
		self.will_start_at = Some(0.0);

		tracing::debug!(name = self.track.name, "starting track at part: {idx}");

		if !self.finished {
			self.manifest_refresh = ManifestRefresh::Part(idx);
		}
	}

	pub fn range_duration(&self) -> Option<&TimeRegion> {
		if self.player_time == -1.0 {
			return None;
		}

		self.requested_regions.get(self.player_time)
	}

	pub fn rendition_info(&self, rendition: &str) -> Option<&RenditionPlaylistRendition> {
		self.manifest.as_ref()?.0.renditions.iter().find(|r| r.name == rendition)
	}

	pub fn drive(&mut self, inner: &PlayerInnerHolder, time: f64) -> PlayerResult<Option<TrackResult>> {
		if self.stopped {
			tracing::debug!(name = self.track.name, "starting track");
		}

		self.stopped = false;

		let dvr_enabled = inner.borrow().use_dvr(self.manifest.is_none());

		let low_latency_enabled = inner.borrow().interface_settings.player_settings.enable_low_latency;

		if (low_latency_enabled && self.last_requested_segment_idx.is_some())
			|| (!low_latency_enabled && self.last_requested_part_idx.is_some())
		{
			tracing::debug!(name = self.track.name, "resetting requests, due to low latency being toggled");
			self.requests.clear();
			self.requested_regions.clear();
			self.last_requested_part_idx = None;
			self.last_requested_segment_idx = None;
		}

		if time != self.player_time {
			if (time - self.player_time).abs() > 10.0 {
				tracing::debug!(
					name = self.track.name,
					"resetting requests, due to large seeking track from {} to {time}",
					self.player_time,
				);
				self.requests.clear();
				self.requested_regions.clear();
				self.last_requested_part_idx = None;
				self.last_requested_segment_idx = None;
			} else {
				tracing::trace!(
					name = self.track.name,
					dvr_enabled,
					low_latency_enabled,
					"driving track from {player_time} to {time}",
					player_time = self.player_time,
					time = time,
				);
			}

			self.player_time = time;
		}

		if !dvr_enabled && !self.segment_regions.is_empty() {
			tracing::debug!(name = self.track.name, "resetting segment regions, due to dvr being disabled");
			self.segment_regions.clear();
		}

		self.drive_manifest(inner, dvr_enabled)?;
		self.drive_init(inner)?;

		if self.manifest.is_none() || self.init_segment.is_none() {
			tracing::trace!(name = self.track.name, "manifest or init segment is none, returning");

			return Ok(None);
		}

		if let Some(result) = self.drive_requests(inner)? {
			tracing::trace!(name = self.track.name, "returning media");
			return Ok(Some(result));
		}

		self.drive_media(inner, low_latency_enabled)
	}

	pub fn drive_requests(&mut self, inner: &PlayerInnerHolder) -> PlayerResult<Option<TrackResult>> {
		if let Some(mut req) = self.requests.done() {
			let result = match req.inflight.as_mut().unwrap().result() {
				Ok(Some(result)) => result,
				Ok(None) => unreachable!("request is done but result is none"),
				Err(FetchError::StatusCode(status, resp)) => {
					if status == 404 {
						// There is a special edge case for 404 responses
						// We need to check if the body is a part finished response
						#[derive(serde::Deserialize)]
						struct Response {
							pub finished: bool,
						}

						if let Ok(resp) = serde_json::from_slice::<Response>(&resp) {
							if resp.finished {
								self.manifest_refresh = ManifestRefresh::Time(now());
								self.finished = true;
								self.requests.clear();
								self.requested_regions.clear();

								let playlist = &mut self.manifest.as_mut().unwrap().0;
								let len = playlist.pre_fetch_part_ids.len();
								playlist.last_pre_fetch_part_idx =
									playlist.last_pre_fetch_part_idx.saturating_sub(len as u32);
								playlist.pre_fetch_part_ids.clear();

								return Ok(None);
							}
						}
					}

					return Err(FetchError::StatusCode(status, resp)).into_event_error(false);
				}
				Err(e) => {
					self.requests.requeue(req);
					return Err(e).into_event_error(false);
				}
			};

			let mut cursor = std::io::Cursor::new(Bytes::from(result));

			let moof = match mp4::DynBox::demux(&mut cursor) {
				Ok(mp4::DynBox::Moof(moof)) => moof,
				Ok(mp4) => {
					tracing::error!("invalid media: expected moof box got {}", mp4.name());
					return Err(EventError::new(
						ErrorCode::Decode,
						format!("invalid media: expected moof box got {}", mp4.name()),
						true,
					));
				}
				Err(err) => {
					// Perhaps the result is a string?
					let data = cursor.into_inner();
					let size = data.len();
					let result = String::from_utf8_lossy(&data);
					tracing::error!("received invalid media: {err}: {result} - {size}");
					return Err(EventError::new(
						ErrorCode::Decode,
						format!("failed to demux media: {}", err),
						true,
					));
				}
			};

			let traf = moof.traf.first().unwrap();
			let decode_time = traf.tfdt.as_ref().unwrap().base_media_decode_time;
			let duration = traf.duration();
			let end_time = decode_time + duration as u64;

			let start_time = decode_time as f64 / self.timescale as f64;
			let end_time = end_time as f64 / self.timescale as f64;

			if let Some(metrics) = req.inflight.as_ref().unwrap().metrics(end_time - start_time) {
				inner.borrow_mut().bandwidth.sample(&metrics)
			}

			return Ok(Some(TrackResult::Media {
				data: cursor.into_inner(),
				start_time,
				end_time,
				decode_time,
				duration,
			}));
		} else {
			self.requests
				.start(&inner.borrow().runner_settings.request_wakeup)
				.into_event_error(false)?;
		}

		Ok(None)
	}

	pub fn drive_media(
		&mut self,
		inner: &PlayerInnerHolder,
		low_latency_enabled: bool,
	) -> PlayerResult<Option<TrackResult>> {
		if self.start_from.is_some() {
			tracing::trace!(name = self.track.name, "starting from is some, drive media cannot be run");
			return Ok(None);
		}

		match self.segment_regions.get(self.player_time) {
			SegmentRangeResult::Active => {
				let (manifest, _) = self.manifest.as_ref().unwrap();

				let Some(media_client) = self.media_client.as_ref() else {
					return Err(EventError::new(
						ErrorCode::Other,
						"active region requested while media client is none".into(),
						true,
					));
				};

				// We need to load the latest parts or segments
				if low_latency_enabled {
					if let Some(part_idx) = self.last_requested_part_idx {
						if let Some((id, _)) = manifest.part(part_idx + 1) {
							self.requests.push(TrackRequest::new(
								media_client.get_mp4(id),
								requests::RequestIndex::Part { idx: part_idx + 1 },
							));
							tracing::trace!(name = self.track.name, "requesting part: {part_idx}", part_idx = part_idx + 1);
							self.last_requested_part_idx = Some(part_idx + 1);
						}
					} else if let Some(segment) = manifest.segments.get(manifest.segments.len().saturating_sub(1)) {
						if let Some(part) = segment.parts.first() {
							let (idx, _) = manifest.part_idx(&part.id).unwrap();
							self.requests.push(TrackRequest::new(
								media_client.get_mp4(&part.id),
								requests::RequestIndex::Part { idx },
							));
							tracing::trace!(name = self.track.name, "requesting part: {idx}", idx = idx);
							self.last_requested_part_idx = Some(idx);
						}
					}
				} else if let Some(segment_idx) = self.last_requested_segment_idx {
					if let Some(segment) = manifest.segments.iter().find(|s| s.idx > segment_idx) {
						if let Some(id) = segment.id.as_deref() {
							let start_time = segment.start_time.unwrap();
							let end_time = segment.end_time.unwrap();

							self.requests.push(TrackRequest::new(
								media_client.get_mp4(id),
								requests::RequestIndex::Segment {
									idx: segment.idx,
									end_time,
									start_time,
								},
							));
							tracing::trace!(
								name = self.track.name,
								"requesting segment: {segment_idx}",
								segment_idx = segment.idx
							);
							self.last_requested_segment_idx = Some(segment.idx);
							self.requested_regions.add(start_time, end_time);
						}
					}
				} else if let Some(segment) = manifest.segments.get(manifest.segments.len().saturating_sub(2)) {
					if let Some(id) = segment.id.as_deref() {
						self.requests.push(TrackRequest::new(
							media_client.get_mp4(id),
							requests::RequestIndex::Segment {
								idx: segment.idx,
								end_time: segment.end_time.unwrap(),
								start_time: segment.start_time.unwrap(),
							},
						));
						tracing::trace!(
							name = self.track.name,
							"requesting segment: {segment_idx}",
							segment_idx = segment.idx
						);
						self.last_requested_segment_idx = Some(segment.idx);
					}
				}
			}
			SegmentRangeResult::Discontinuity(a, b) => {
				tracing::debug!(
					name = self.track.name,
					"discontinuity: {a} -> {b}",
					a = a.map(|r| r.end).unwrap_or(0.0),
					b = b.map(|r| r.start).unwrap_or(0.0)
				);
				return Ok(Some(TrackResult::Discontinuity(
					a.map(|r| r.end).unwrap_or(0.0),
					b.map(|r| r.start).unwrap_or(0.0),
				)));
			}
			SegmentRangeResult::Range(s) => {
				// IN DVR MODE
				// We have a segment that contains the time that was requested
				// We can load this segment.
				self.last_requested_part_idx = None;
				self.last_requested_segment_idx = None;

				let dvr_prefix = self.dvr_prefix.as_ref().expect("dvr prefix not set");

				if let Some(region) = self.requested_regions.get(s.start) {
					if region.end
						- inner
							.borrow()
							.interface_settings
							.player_settings
							.static_target_buffer_duration_ms
							/ 1000.0 < self.player_time
					{
						if let SegmentRangeResult::Range(next_segment) = self.segment_regions.get(region.end) {
							if next_segment.idx != s.idx {
								self.requests.push(TrackRequest::new(
									FetchRequest::new("GET", dvr_prefix.join(&next_segment.dvr_tag).unwrap()),
									requests::RequestIndex::Segment {
										idx: next_segment.idx,
										end_time: next_segment.end,
										start_time: next_segment.start,
									},
								));

								tracing::trace!(
									name = self.track.name,
									"requesting segment: {segment_idx} ({start} -> {end})",
									segment_idx = next_segment.idx,
									start = next_segment.start,
									end = next_segment.end
								);

								self.requested_regions.add(next_segment.start, next_segment.end);
							}
						}
					}
				} else {
					self.requests.push(TrackRequest::new(
						FetchRequest::new("GET", dvr_prefix.join(&s.dvr_tag).unwrap()),
						requests::RequestIndex::Segment {
							idx: s.idx,
							start_time: s.start,
							end_time: s.end,
						},
					));
					tracing::trace!(
						name = self.track.name,
						"requesting segment: {segment_idx} ({start} -> {end})",
						segment_idx = s.idx,
						start = s.start,
						end = s.end
					);
					self.requested_regions.add(s.start, s.end);
				}
			}
		};

		Ok(None)
	}

	pub fn drive_manifest(&mut self, inner: &PlayerInnerHolder, dvr_enabled: bool) -> PlayerResult<()> {
		if !self.finished
			&& self
				.manifest
				.as_ref()
				.map(|(_, time)| now() - *time > 6000.0)
				.unwrap_or_default()
		{
			tracing::debug!(name = self.track.name, "manifest is stale, resetting manifest");
			self.manifest = None;
			self.manifest_request = None;
			self.requests.clear();
			self.requested_regions.clear();
			self.segment_regions.clear();
			self.last_requested_part_idx = None;
			self.last_requested_segment_idx = None;
		}

		if let Some(request) = self.manifest_request.as_mut() {
			request
				.req
				.start(&inner.borrow().runner_settings.request_wakeup)
				.into_event_error(false)
				.map_err(|mut err| {
					self.manifest_req_error_count += 1;
					tracing::warn!(
						name = self.track.name,
						"failed to fetch playlist ({count})",
						count = self.manifest_req_error_count
					);
					self.manifest_refresh =
						ManifestRefresh::Time(now() + (self.manifest_req_error_count - 1) as f64 * ERROR_BACKOFF);

					err.set_fatal(self.manifest_req_error_count > MAX_ERROR_COUNT);

					err
				})?;

			if now() - request.requested_at > 6000.0 {
				tracing::warn!(
					name = self.track.name,
					"manifest request has been inflight for more than 2 seconds, resetting manifest"
				);
				self.manifest_request = None;
				self.manifest_req_error_count += 1;
				self.manifest_refresh =
					ManifestRefresh::Time(now() + (self.manifest_req_error_count - 1) as f64 * ERROR_BACKOFF);

				if self.manifest_req_error_count > MAX_ERROR_COUNT {
					return Err(EventError::new(
						ErrorCode::Other,
						"failed to fetch playlist too many times".into(),
						true,
					));
				} else {
					return Err(EventError::new(
						ErrorCode::Other,
						"failed to fetch playlist, request took too long".into(),
						false,
					));
				}
			} else if request.req.is_done() {
				let mut request = self.manifest_request.take().unwrap();
				let result = request
					.req
					.json(&inner.borrow().runner_settings.request_wakeup)
					.into_event_error(false)
					.map_err(|mut err| {
						self.manifest_req_error_count += 1;
						tracing::warn!(
							name = self.track.name,
							"failed to fetch playlist ({count})",
							count = self.manifest_req_error_count
						);
						self.manifest_refresh =
							ManifestRefresh::Time(now() + (self.manifest_req_error_count - 1) as f64 * ERROR_BACKOFF);

						err.set_fatal(self.manifest_req_error_count > MAX_ERROR_COUNT);

						err
					})?
					.unwrap();

				self.handle_manifest(inner, result, request.was_dvr)?;
			}
		} else if self.manifest.is_none() {
			if self.manifest_req_error_count > MAX_ERROR_COUNT {
				return Err(EventError::new(
					ErrorCode::Other,
					"failed to fetch playlist too many times".into(),
					true,
				));
			}

			let hls_skip = !self.segment_regions.is_empty();
			let scuffle_part = self.start_from.and_then(|s| match s {
				StartFrom::IPart(idx) => Some(ScufflePart::IPart(idx)),
				_ => None,
			});

			let mut req = self.client.get_rendition(
				&self.track.name,
				&RenditionSettings {
					hls_skip,
					scuffle_dvr: dvr_enabled,
					scuffle_part,
				},
			);

			tracing::debug!(
				name = self.track.name,
				dvr_enabled,
				hls_skip,
				?scuffle_part,
				"fetching playlist"
			);

			req.start(&inner.borrow().runner_settings.request_wakeup)
				.into_event_error(false)
				.map_err(|mut err| {
					self.manifest_req_error_count += 1;
					tracing::warn!(
						name = self.track.name,
						"failed to fetch playlist ({count})",
						count = self.manifest_req_error_count
					);
					self.manifest_refresh =
						ManifestRefresh::Time(now() + (self.manifest_req_error_count - 1) as f64 * ERROR_BACKOFF);

					err.set_fatal(self.manifest_req_error_count > MAX_ERROR_COUNT);

					err
				})?;

			self.manifest_request = Some(ManifestRequest {
				req,
				requested_at: now(),
				was_dvr: dvr_enabled,
			});
			self.last_session_refreshed = now();
		} else {
			match self.manifest_refresh {
				ManifestRefresh::Never => {}
				ManifestRefresh::IPart(idx) => {
					// if let Some(last_fetched_part_idx) = self.last_fetched_part_idx {
					//     tracing::debug!(
					//         name = self.track.name,
					//         "last fetched part idx is greater than requested part idx, requesting
					// manifest"     );
					self.manifest_request = Some(ManifestRequest {
						req: self.client.get_rendition(
							&self.track.name,
							&RenditionSettings {
								hls_skip: !self.segment_regions.is_empty(),
								scuffle_dvr: dvr_enabled,
								scuffle_part: Some(ScufflePart::IPart(idx)),
							},
						),
						requested_at: now(),
						was_dvr: dvr_enabled,
					});
					self.last_session_refreshed = now();
				}
				ManifestRefresh::Part(idx) => {
					self.manifest_request = Some(ManifestRequest {
						req: self.client.get_rendition(
							&self.track.name,
							&RenditionSettings {
								hls_skip: !self.segment_regions.is_empty(),
								scuffle_dvr: dvr_enabled,
								scuffle_part: Some(ScufflePart::Part(idx)),
							},
						),
						requested_at: now(),
						was_dvr: dvr_enabled,
					});
					self.last_session_refreshed = now();
				}
				ManifestRefresh::Time(time) => {
					if time < now() {
						tracing::debug!(
							name = self.track.name,
							"manifest refresh time is less than now, requesting manifest"
						);
						self.manifest_request = Some(ManifestRequest {
							req: self.client.get_rendition(
								&self.track.name,
								&RenditionSettings {
									hls_skip: !self.segment_regions.is_empty(),
									scuffle_dvr: dvr_enabled,
									scuffle_part: None,
								},
							),
							requested_at: now(),
							was_dvr: dvr_enabled,
						});
						self.last_session_refreshed = now();
					}
				}
			}
		}

		Ok(())
	}

	pub fn drive_init(&mut self, inner: &PlayerInnerHolder) -> PlayerResult<()> {
		if self.init_segment.is_some() {
			return Ok(());
		}

		if self.next_init_req_time > now() {
			tracing::trace!(name = self.track.name, "next init req time is greater than now, returning");

			return Ok(());
		}

		if let Some(request) = self.init_request.as_mut() {
			if request.inflight.as_ref().map(|i| i.is_done()).unwrap_or_default() {
				let Some(init) = self
					.init_request
					.take()
					.unwrap()
					.inflight
					.unwrap()
					.result()
					.into_event_error(false)
					.map_err(|mut err| {
						self.init_req_error_count += 1;
						tracing::warn!(
							name = self.track.name,
							"failed to fetch init segment ({count})",
							count = self.init_req_error_count
						);
						self.next_init_req_time = now() + (self.init_req_error_count - 1) as f64 * ERROR_BACKOFF;

						err.set_fatal(self.init_req_error_count > MAX_ERROR_COUNT);

						err
					})?
				else {
					return Err(EventError::new(ErrorCode::Other, "failed to fetch init segment".into(), true));
				};

				let mut cursor = std::io::Cursor::new(Bytes::from(init));

				match mp4::DynBox::demux(&mut cursor) {
					Ok(mp4::DynBox::Ftyp(_)) => {}
					Ok(mp4) => {
						tracing::error!("invalid init segment: expected ftyp box got {}", mp4.name());
						return Err(EventError::new(
							ErrorCode::Decode,
							format!("invalid init segment: expected ftyp box got {}", mp4.name()),
							true,
						));
					}
					Err(err) => {
						let data = cursor.into_inner();
						let size = data.len();
						let result = String::from_utf8_lossy(&data);
						tracing::error!("received invalid init segment: {err}: {result} - {size}");
						return Err(EventError::new(
							ErrorCode::Decode,
							format!("failed to demux init segment: {}", err),
							true,
						));
					}
				};

				let moov = match mp4::DynBox::demux(&mut cursor) {
					Ok(mp4::DynBox::Moov(moov)) => moov,
					Ok(mp4) => {
						tracing::error!("invalid init segment: expected moov box got {}", mp4.name());
						return Err(EventError::new(
							ErrorCode::Decode,
							format!("invalid init segment: expected moov box got {}", mp4.name()),
							true,
						));
					}
					Err(err) => {
						tracing::error!("failed to demux init segment: {}", err);
						return Err(EventError::new(
							ErrorCode::Decode,
							format!("failed to demux init segment: {}", err),
							true,
						));
					}
				};

				self.timescale = moov.traks.first().unwrap().mdia.mdhd.timescale;
				tracing::debug!(name = self.track.name, timescale = self.timescale, "init segment loaded");
				self.init_segment = Some(cursor.into_inner());
			}
		} else if let Some((manifest, _)) = self.manifest.as_ref() {
			tracing::debug!(name = self.track.name, "fetching init segment");

			let req = if manifest.init_dvr {
				FetchRequest::new(
					"GET",
					self.dvr_prefix.as_ref().unwrap().join(&manifest.init_segment_id).unwrap(),
				)
			} else {
				self.media_client.as_ref().unwrap().get_mp4(&manifest.init_segment_id)
			};

			self.init_request = Some(
				TrackRequest::new_with_start(
					req,
					requests::RequestIndex::Init,
					&inner.borrow().runner_settings.request_wakeup,
				)
				.into_event_error(false)
				.map_err(|mut err| {
					self.init_req_error_count += 1;
					tracing::warn!(
						name = self.track.name,
						"failed to fetch init segment ({count})",
						count = self.init_req_error_count
					);
					self.next_init_req_time = now() + (self.init_req_error_count - 1) as f64 * ERROR_BACKOFF;

					err.set_fatal(self.init_req_error_count > MAX_ERROR_COUNT);

					err
				})?,
			);
		}

		Ok(())
	}

	pub fn handle_manifest(
		&mut self,
		inner: &PlayerInnerHolder,
		manifest: RenditionPlaylist,
		was_dvr: bool,
	) -> PlayerResult<()> {
		self.manifest_req_error_count = 0;

		if let Some(mut dvr_prefix) = manifest.dvr_prefix.clone() {
			if !dvr_prefix.path().ends_with('/') {
				dvr_prefix.set_path(format!("{}/", dvr_prefix.path()).as_str());
			}

			self.dvr_prefix = Some(dvr_prefix);
		}

		if self.init_segment.is_none() && self.init_request.is_none() {
			let req = if manifest.init_dvr {
				FetchRequest::new(
					"GET",
					self.dvr_prefix.as_ref().unwrap().join(&manifest.init_segment_id).unwrap(),
				)
			} else {
				self.media_client.as_ref().unwrap().get_mp4(&manifest.init_segment_id)
			};

			self.init_request = Some(
				TrackRequest::new_with_start(
					req,
					requests::RequestIndex::Init,
					&inner.borrow().runner_settings.request_wakeup,
				)
				.into_event_error(false)
				.map_err(|mut err| {
					self.init_req_error_count += 1;
					tracing::warn!(
						name = self.track.name,
						"failed to fetch init segment ({count})",
						count = self.init_req_error_count
					);
					self.next_init_req_time = now() + (self.init_req_error_count - 1) as f64 * ERROR_BACKOFF;

					err.set_fatal(self.init_req_error_count > MAX_ERROR_COUNT);

					err
				})?,
			);
		}

		if let Some(start_from) = self.start_from.take() {
			match start_from {
				StartFrom::IPart(_) => {
					let part = manifest
						.segments
						.iter()
						.rev()
						.flat_map(|s| s.parts.iter().rev())
						.find(|p| p.independent)
						.unwrap();
					self.last_requested_part_idx = manifest.part_idx(&part.id).unwrap().0.checked_sub(1);
					let segment = manifest
						.segments
						.iter()
						.rev()
						.find(|s| s.parts.iter().rev().any(|p| p.id == part.id))
						.unwrap();
					self.will_start_at = Some(
						segment.start_time.unwrap()
							+ segment
								.parts
								.iter()
								.take_while(|p| p.id != part.id)
								.map(|p| p.duration)
								.sum::<f64>(),
					);
				}
				StartFrom::Time(time) => {
					self.requested_regions.add(self.player_time, time);
					self.will_start_at = Some(time);
				}
			}
		}

		if was_dvr {
			let mut inner = inner.borrow_mut();
			inner.runner_settings.dvr_supported = manifest.dvr_prefix.is_some();
			inner.runner_settings.thumbnail_prefix.clone_from(&manifest.thumbnail_prefix);

			manifest.thumbnails.iter().for_each(|t| {
				match inner.runner_settings.thumbnails.binary_search_by(|a| a.idx.cmp(&t.idx)) {
					Ok(idx) => {
						inner.runner_settings.thumbnails[idx] = t.clone();
					}
					Err(idx) => {
						inner.runner_settings.thumbnails.insert(idx, t.clone());
					}
				}
			});
		}

		for segment in manifest.segments.iter() {
			self.segment_regions.add(segment);
		}

		if manifest.finished {
			self.segment_regions
				.active_range(manifest.segments.last().as_mut().unwrap().end_time.unwrap() + 1.0);
		} else if !manifest.segments.is_empty() {
			if let Some(active_segment) = manifest.segments.iter().find(|s| s.id.is_some()) {
				self.segment_regions.active_range(active_segment.start_time.unwrap());
			}
		}

		if manifest.finished {
			self.manifest_refresh = ManifestRefresh::Never;
			self.finished = true;
		} else {
			self.manifest_refresh = ManifestRefresh::Time(now() + 2000.0);
		}

		self.manifest = Some((manifest, now()));

		Ok(())
	}
}
