use rtmp::ChannelData;

#[derive(Debug, Default)]
pub struct BytesTracker {
	video: u64,
	audio: u64,
	metadata: u64,
	since_keyframe: u64,
}

impl BytesTracker {
	pub fn add(&mut self, data: &ChannelData) {
		match data {
			ChannelData::Video { data, .. } => self.video += data.len() as u64,
			ChannelData::Audio { data, .. } => self.audio += data.len() as u64,
			ChannelData::Metadata { data, .. } => self.metadata += data.len() as u64,
		}

		self.since_keyframe += data.data().len() as u64;
	}

	pub fn total(&self) -> u64 {
		self.video + self.audio + self.metadata
	}

	pub fn keyframe(&mut self) {
		self.since_keyframe = 0;
	}

	pub fn since_keyframe(&self) -> u64 {
		self.since_keyframe
	}

	pub fn clear(&mut self) {
		self.video = 0;
		self.audio = 0;
		self.metadata = 0;
	}
}
