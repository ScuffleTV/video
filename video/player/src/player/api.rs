use std::rc::Rc;

use tokio::sync::broadcast;
use ulid::Ulid;
use url::Url;
use video_player_types::{RenditionPlaylist, SessionPlaylist, SessionRefresh};

use super::fetch::{FetchError, FetchRequest, FetchResult, InflightRequest};

#[derive(Debug, Clone)]
pub struct ApiClient {
	server: Rc<Url>,
}

#[derive(Debug, Clone)]
pub struct ApiSessionClient {
	server: Rc<Url>,
}

#[derive(Debug, Clone)]
pub struct ApiMediaClient {
	server: Rc<Url>,
}

pub struct Json<T: serde::de::DeserializeOwned> {
	pub req: FetchRequest,
	pub inflight: Option<InflightRequest>,
	_phantom: std::marker::PhantomData<T>,
}

impl<T: serde::de::DeserializeOwned> std::fmt::Debug for Json<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Json")
			.field("url", &self.req.url())
			.field("inflight", &self.inflight.is_some())
			.finish()
	}
}

impl<T: serde::de::DeserializeOwned> Json<T> {
	fn new(req: FetchRequest) -> Self {
		Self {
			req,
			inflight: None,
			_phantom: std::marker::PhantomData,
		}
	}

	pub fn is_done(&self) -> bool {
		self.inflight.as_ref().map(|i| i.is_done()).unwrap_or_default()
	}

	pub fn start(&mut self, wakeup: &broadcast::Sender<()>) -> FetchResult<()> {
		if self.inflight.is_none() {
			self.inflight = Some(self.req.start(wakeup.clone())?);
		}

		Ok(())
	}

	pub fn json(&mut self, wakeup: &broadcast::Sender<()>) -> FetchResult<Option<T>> {
		self.start(wakeup)?;

		let Some(resp) = self.inflight.as_mut().unwrap().result()? else {
			return Ok(None);
		};

		let json = serde_json::from_slice(resp.as_slice()).map_err(FetchError::Json)?;

		Ok(json)
	}

	pub async fn wait_json(&mut self, wakeup: &broadcast::Sender<()>) -> FetchResult<T> {
		self.start(wakeup)?;

		let resp = self.inflight.as_mut().unwrap().wait_result().await?;

		let json = serde_json::from_slice(resp.as_slice()).map_err(FetchError::Json)?;

		Ok(json)
	}
}

impl ApiClient {
	pub fn new(server: Url, organization_id: Ulid) -> Self {
		Self {
			server: Rc::new(server.join(format!("{organization_id}/").as_str()).unwrap()),
		}
	}

	pub fn get_room(&self, room_id: Ulid, token: Option<&str>) -> Json<SessionPlaylist> {
		let mut url = self.server.join(format!("{}.m3u8", room_id).as_str()).unwrap();

		url.query_pairs_mut().append_pair("_SCUFFLE_json", "YES");

		if let Some(token) = token {
			url.query_pairs_mut().append_pair("token", token);
		}

		let req = FetchRequest::new("GET", url);

		Json::new(req)
	}

	pub fn get_recording(&self, recording_id: Ulid, token: Option<&str>) -> Json<SessionPlaylist> {
		let mut url = self.server.join(format!("r/{}.m3u8", recording_id).as_str()).unwrap();

		url.query_pairs_mut().append_pair("_SCUFFLE_json", "YES");

		if let Some(token) = token {
			url.query_pairs_mut().append_pair("token", token);
		}

		let req = FetchRequest::new("GET", url);

		Json::new(req)
	}

	pub fn session_client(&self, resp: &SessionPlaylist) -> ApiSessionClient {
		ApiSessionClient {
			server: Rc::new(
				self.server
					.join(format!("{session}/", session = resp.session).as_str())
					.unwrap(),
			),
		}
	}

	pub fn media_client(&self, room_id: Ulid) -> ApiMediaClient {
		ApiMediaClient {
			server: Rc::new(self.server.join(format!("{room_id}/").as_str()).unwrap()),
		}
	}
}

#[derive(Default, Debug, Clone)]
pub struct RenditionSettings {
	pub scuffle_part: Option<ScufflePart>,
	pub scuffle_dvr: bool,
	pub hls_skip: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum ScufflePart {
	// Waits until this part is available
	Part(u32),
	// Waits until an independent part is available, where the id is greater than this value.
	IPart(u32),
}

impl ApiSessionClient {
	pub fn get_rendition(&self, rendition: &str, settings: &RenditionSettings) -> Json<RenditionPlaylist> {
		let mut url = self.server.join(format!("{}.m3u8", rendition).as_str()).unwrap();

		url.query_pairs_mut().append_pair("_SCUFFLE_json", "YES");

		if let Some(scuffle_part) = &settings.scuffle_part {
			match scuffle_part {
				ScufflePart::Part(part) => {
					url.query_pairs_mut().append_pair("_SCUFFLE_part", part.to_string().as_str());
				}
				ScufflePart::IPart(part) => {
					url.query_pairs_mut().append_pair("_SCUFFLE_ipart", part.to_string().as_str());
				}
			}
		}

		if settings.scuffle_dvr {
			url.query_pairs_mut().append_pair("_SCUFFLE_dvr", "YES");
		}

		if settings.hls_skip {
			url.query_pairs_mut().append_pair("_HLS_skip", "YES");
		}

		let req = FetchRequest::new("GET", url);

		Json::new(req)
	}

	pub fn refresh(&self) -> Json<SessionRefresh> {
		Json::new(FetchRequest::new("GET", self.server.join("refresh").unwrap()))
	}
}

impl ApiMediaClient {
	pub fn get_mp4(&self, id: &str) -> FetchRequest {
		FetchRequest::new("GET", self.server.join(format!("{id}.mp4").as_str()).unwrap())
	}
}
