use ulid::Ulid;
use url::Url;

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct RenditionPlaylist {
	#[serde(rename = "s")]
	pub segments: Vec<RenditionPlaylistSegment>,
	#[serde(rename = "pl")]
	pub pre_fetch_part_ids: Vec<String>,
	#[serde(rename = "lpi")]
	pub last_pre_fetch_part_idx: u32,
	#[serde(rename = "init")]
	pub init_segment_id: String,
	#[serde(rename = "init_dvr")]
	pub init_dvr: bool,
	#[serde(rename = "f", default, skip_serializing_if = "is_false")]
	pub finished: bool,
	#[serde(rename = "r")]
	pub renditions: Vec<RenditionPlaylistRendition>,
	#[serde(rename = "dp", default, skip_serializing_if = "Option::is_none")]
	pub dvr_prefix: Option<Url>,
	#[serde(rename = "tp", default, skip_serializing_if = "Option::is_none")]
	pub thumbnail_prefix: Option<Url>,
	#[serde(rename = "sr", default, skip_serializing_if = "Vec::is_empty")]
	pub thumbnails: Vec<ThumbnailRange>,

	#[serde(skip)]
	pub msn: u32,
	#[serde(skip)]
	pub skip_segments: u32,
}

impl RenditionPlaylist {
	pub fn to_m3u8(&self, organization_id: Ulid, room_id: Option<Ulid>) -> String {
		let mut m3u8 = String::new();

		m3u8.push_str("#EXTM3U\n");
		if room_id.is_none() {
			m3u8.push_str("#EXT-X-VERSION:6\n");
		} else if self.dvr_prefix.is_some() {
			m3u8.push_str("#EXT-X-VERSION:9\n");
		} else {
			m3u8.push_str("#EXT-X-VERSION:7\n");
		}

		m3u8.push_str("#EXT-X-TARGETDURATION:5\n");

		if room_id.is_none() {
			m3u8.push_str("#EXT-X-PLAYLIST-TYPE:VOD\n");
		} else if self.dvr_prefix.is_some() {
			m3u8.push_str("#EXT-X-PLAYLIST-TYPE:EVENT\n");
		}

		m3u8.push_str(format!("#EXT-X-MEDIA-SEQUENCE:{}\n", self.msn).as_str());

		m3u8.push_str("#EXT-DISCONTINUITY-SEQUENCE:0\n");
		if room_id.is_some() {
			m3u8.push_str("#EXT-X-PART-INF:PART-TARGET=0.250\n");
			if !self.finished {
				m3u8.push_str("#EXT-X-SERVER-CONTROL:PART-HOLD-BACK=0.750,CAN-BLOCK-RELOAD=YES");
				if self.dvr_prefix.is_some() {
					m3u8.push_str(",SKIP-UNTIL=15\n");
				} else {
					m3u8.push('\n');
				}
			}
		}

		if self.dvr_prefix.is_some() {
			m3u8.push_str(format!("#EXT-X-SKIP:SKIPPED-SEGMENTS={}\n", self.skip_segments).as_str());
		}

		if let Some(room_id) = room_id {
			m3u8.push_str(
				format!(
					"#EXT-X-MAP:URI=\"/{organization_id}/{room_id}/{}.mp4\"\n",
					self.init_segment_id
				)
				.as_str(),
			);
		} else {
			m3u8.push_str(
				format!(
					"#EXT-X-MAP:URI=\"{}/{}\"\n",
					self.dvr_prefix.as_ref().unwrap(),
					self.init_segment_id
				)
				.as_str(),
			);
		}

		for segment in self.segments.iter() {
			for part in segment.parts.iter() {
				m3u8.push_str(
					format!(
						"#EXT-X-PART:DURATION={:.3},URI=\"/{organization_id}/{room_id}/{}.mp4\"",
						part.duration,
						part.id,
						room_id = room_id.unwrap(),
					)
					.as_str(),
				);
				if part.independent {
					m3u8.push_str(",INDEPENDENT=YES\n");
				} else {
					m3u8.push('\n');
				}
			}

			if let Some(id) = segment.id.as_ref() {
				if let Some(dvr_prefix) = self.dvr_prefix.as_ref() {
					if let Some(dvr_tag) = segment.dvr_tag.as_ref() {
						m3u8.push_str(format!("#EXT-X-SCUFFLE-DVR:URI=\"{dvr_prefix}/{dvr_tag}\"\n").as_str());
					}
				}
				m3u8.push_str(format!("#EXTINF:{:.3},\n", segment.duration()).as_str());
				m3u8.push_str(format!("/{organization_id}/{room_id}/{id}.mp4\n", room_id = room_id.unwrap()).as_str());
			} else if let Some(dvr_tag) = segment.dvr_tag.as_ref() {
				if let Some(dvr_prefix) = self.dvr_prefix.as_ref() {
					m3u8.push_str(format!("#EXTINF:{:.3},\n", segment.duration()).as_str());
					m3u8.push_str(format!("{dvr_prefix}/{dvr_tag}\n").as_str());
				}
			}
		}

		let prefetch_length = self.pre_fetch_part_ids.len();
		let first_prefetch_idx = self.last_pre_fetch_part_idx + 1 - prefetch_length as u32;

		if self.finished {
			m3u8.push_str("#EXT-X-ENDLIST\n");
		} else if let Some(room_id) = room_id {
			for (idx, part_id) in self.pre_fetch_part_ids.iter().enumerate() {
				let idx = idx as u32 + first_prefetch_idx;
				m3u8.push_str(
					format!(
						"#EXT-X-PRELOAD-HINT:TYPE=PART,SCUFFLE-PART={idx},URI=\"/{organization_id}/{room_id}/{part_id}.mp4\"\n"
					)
					.as_str(),
				);
			}

			for rendition in self.renditions.iter() {
				m3u8.push_str(
					format!(
						"#EXT-X-RENDITION-REPORT:URI=\"./{}.m3u8\",LAST-MSN={},LAST-PART={}\n",
						rendition.name, rendition.last_segment_idx, rendition.last_segment_part_idx
					)
					.as_str(),
				);
			}
		}

		m3u8
	}

	pub fn part_idx(&self, id: &str) -> Option<(u32, bool)> {
		let parts = self
			.segments
			.iter()
			.flat_map(|x| x.parts.iter().map(|p| (p.id.as_str(), false)))
			.chain(self.pre_fetch_part_ids.iter().map(|p| (p.as_str(), true)))
			.collect::<Vec<_>>();

		parts
			.iter()
			.enumerate()
			.find(|(_, (x, _))| x == &id)
			.map(|(x, (_, prefetch))| (x as u32 + self.last_pre_fetch_part_idx + 1 - parts.len() as u32, *prefetch))
	}

	pub fn part(&self, idx: u32) -> Option<(&str, bool)> {
		let parts = self
			.segments
			.iter()
			.flat_map(|x| x.parts.iter().map(|p| (p.id.as_str(), false)))
			.chain(self.pre_fetch_part_ids.iter().map(|p| (p.as_str(), true)))
			.collect::<Vec<_>>();

		let start_idx = self.last_pre_fetch_part_idx + 1 - parts.len() as u32;
		if idx < start_idx {
			return parts.first().copied();
		}

		parts.get((idx - start_idx) as usize).copied()
	}
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RenditionPlaylistRendition {
	#[serde(rename = "n")]
	pub name: String,
	#[serde(rename = "lsi")]
	pub last_segment_idx: u32,
	#[serde(rename = "lipi")]
	pub last_independent_part_idx: u32,

	#[serde(skip)]
	pub last_segment_part_idx: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RenditionPlaylistSegment {
	#[serde(rename = "i", default, skip_serializing_if = "Option::is_none")]
	pub id: Option<String>,

	#[serde(rename = "s", default, skip_serializing_if = "Option::is_none")]
	pub start_time: Option<f64>,

	#[serde(rename = "e", default, skip_serializing_if = "Option::is_none")]
	pub end_time: Option<f64>,

	#[serde(rename = "x")]
	pub idx: u32,

	#[serde(rename = "d", default, skip_serializing_if = "Option::is_none")]
	pub dvr_tag: Option<String>,

	#[serde(rename = "p", default, skip_serializing_if = "Vec::is_empty")]
	pub parts: Vec<RenditionPlaylistSegmentPart>,
}

impl RenditionPlaylistSegment {
	pub fn duration(&self) -> f64 {
		if self.discontinuity() {
			return 0.0;
		}

		if let Some(et) = self.end_time {
			et - self.start_time.unwrap()
		} else {
			self.parts.iter().map(|x| x.duration).sum()
		}
	}

	pub fn discontinuity(&self) -> bool {
		self.start_time.is_none()
	}
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RenditionPlaylistSegmentPart {
	#[serde(rename = "i")]
	pub id: String,
	#[serde(rename = "d")]
	pub duration: f64,
	#[serde(rename = "k", default, skip_serializing_if = "is_false")]
	pub independent: bool,
}

fn is_false(b: &bool) -> bool {
	!b
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ThumbnailRange {
	#[serde(rename = "n")]
	pub idx: u32,
	#[serde(rename = "i")]
	pub id: String,
	#[serde(rename = "t")]
	pub start_time: f64,
}
