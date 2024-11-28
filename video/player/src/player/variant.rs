use video_player_types::{RoomPlaylistTrack, RoomPlaylistTrackAudio, RoomPlaylistTrackVideo};

#[derive(Debug, Clone, tsify::Tsify, serde::Serialize)]
pub struct Variant {
	pub audio_track: AudioTrack,
	pub video_track: Option<VideoTrack>,
}

impl Variant {
	pub fn new(
		audio_track: (usize, &RoomPlaylistTrack<RoomPlaylistTrackAudio>),
		video_track: Option<(usize, &RoomPlaylistTrack<RoomPlaylistTrackVideo>)>,
	) -> Self {
		Self {
			audio_track: AudioTrack {
				id: audio_track.0,
				name: audio_track.1.name.clone(),
				codec: audio_track.1.codec.clone(),
				channels: audio_track.1.other.channels,
				sample_rate: audio_track.1.other.sample_rate,
				bitrate: audio_track.1.bitrate,
			},
			video_track: video_track.map(|(id, track)| VideoTrack {
				id,
				name: track.name.clone(),
				codec: track.codec.clone(),
				width: track.other.width,
				height: track.other.height,
				frame_rate: track.other.frame_rate,
				bitrate: track.bitrate,
			}),
		}
	}
}

#[derive(Debug, Clone, tsify::Tsify, serde::Serialize)]
pub struct AudioTrack {
	pub id: usize,
	pub name: String,
	pub codec: String,
	pub channels: u32,
	pub sample_rate: u32,
	pub bitrate: u32,
}

#[derive(Debug, Clone, tsify::Tsify, serde::Serialize)]
pub struct VideoTrack {
	pub id: usize,
	pub name: String,
	pub codec: String,
	pub width: u32,
	pub height: u32,
	pub frame_rate: u32,
	pub bitrate: u32,
}
