use ulid::Ulid;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SessionPlaylist {
	#[serde(rename = "v")]
	pub video_tracks: Vec<RoomPlaylistTrack<RoomPlaylistTrackVideo>>,
	#[serde(rename = "a")]
	pub audio_tracks: Vec<RoomPlaylistTrack<RoomPlaylistTrackAudio>>,
	#[serde(rename = "s")]
	pub session: String,
}

impl SessionPlaylist {
	pub fn to_m3u8(&self, organization_id: Ulid) -> String {
		let mut m3u8 = String::new();

		m3u8.push_str("#EXTM3U\n");
		m3u8.push_str("#EXT-X-INDEPENDENT-SEGMENTS\n");

		for track in self.audio_tracks.iter() {
			m3u8.push_str("#EXT-X-MEDIA:TYPE=AUDIO,");
			m3u8.push_str(format!("GROUP-ID=\"{}\",", track.name).as_str());
			m3u8.push_str("DEFAULT=YES,");
			m3u8.push_str("AUTOSELECT=YES,");
			m3u8.push_str(format!("NAME=\"{}\",", track.name).as_str());
			m3u8.push_str(format!("CHANNELS=\"{}\",", track.other.channels).as_str());
			m3u8.push_str(
				format!(
					"URI=\"/{organization_id}/{session}/{name}.m3u8\"\n",
					organization_id = organization_id,
					session = self.session,
					name = track.name,
				)
				.as_str(),
			);
		}

		for video in self.video_tracks.iter() {
			for audio in self.audio_tracks.iter() {
				m3u8.push_str("#EXT-X-STREAM-INF:");
				m3u8.push_str(format!("BANDWIDTH={},", video.bitrate + audio.bitrate).as_str());
				m3u8.push_str(format!("CODECS=\"{},{}\",", video.codec, audio.codec).as_str());
				m3u8.push_str(format!("RESOLUTION={}x{},", video.other.width, video.other.height).as_str());
				m3u8.push_str(format!("FRAME-RATE={},", video.other.frame_rate).as_str());
				m3u8.push_str(format!("AUDIO=\"{}\"\n", audio.name).as_str());
				m3u8.push_str(
					format!(
						"/{organization_id}/{session}/{name}.m3u8\n",
						organization_id = organization_id,
						session = self.session,
						name = video.name,
					)
					.as_str(),
				);
			}
		}

		m3u8
	}
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RoomPlaylistTrack<T> {
	#[serde(rename = "n")]
	pub name: String,
	#[serde(rename = "br")]
	pub bitrate: u32,
	#[serde(rename = "c")]
	pub codec: String,
	#[serde(flatten)]
	pub other: T,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RoomPlaylistTrackVideo {
	#[serde(rename = "w")]
	pub width: u32,
	#[serde(rename = "h")]
	pub height: u32,
	#[serde(rename = "fr")]
	pub frame_rate: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RoomPlaylistTrackAudio {
	#[serde(rename = "ch")]
	pub channels: u32,
	#[serde(rename = "sr")]
	pub sample_rate: u32,
}
