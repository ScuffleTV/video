#[derive(Debug, PartialEq, Eq, Clone)]

/// RTMP Commands are defined in the RTMP specification
pub(super) enum RtmpCommand {
	/// NetConnection.connect
	Connect,
	/// NetConnection.createStream
	CreateStream,
	/// NetStream.publish
	Publish,
	/// NetStream.play
	Play,
	/// NetStream.deleteStream
	DeleteStream,
	/// NetStream.closeStream
	CloseStream,
	/// NetStream.releaseStream
	ReleaseStream,
	/// Unknown command
	Unknown(String),
}

impl From<&str> for RtmpCommand {
	fn from(command: &str) -> Self {
		match command {
			"connect" => Self::Connect,
			"createStream" => Self::CreateStream,
			"deleteStream" => Self::DeleteStream,
			"publish" => Self::Publish,
			"play" => Self::Play,
			"closeStream" => Self::CloseStream,
			"releaseStream" => Self::ReleaseStream,
			_ => Self::Unknown(command.to_string()),
		}
	}
}
