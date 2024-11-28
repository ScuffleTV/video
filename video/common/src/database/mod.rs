mod access_token;
mod organization;
mod playback_key_pair;
mod playback_session;
mod playback_session_browser;
mod playback_session_device;
mod playback_session_platform;
mod recording;
mod recording_config;
mod recording_rendition;
mod recording_rendition_segment;
mod recording_thumbnail;
mod rendition;
mod room;
mod room_status;
mod s3_bucket;
mod session_token_revoke;
mod transcoding_config;
mod visibility;

pub use access_token::*;
pub use organization::*;
pub use playback_key_pair::*;
pub use playback_session::*;
pub use playback_session_browser::*;
pub use playback_session_device::*;
pub use playback_session_platform::*;
pub use recording::*;
pub use recording_config::*;
pub use recording_rendition::*;
pub use recording_rendition_segment::*;
pub use recording_thumbnail::*;
pub use rendition::*;
pub use room::*;
pub use room_status::*;
pub use s3_bucket::*;
pub use session_token_revoke::*;
pub use transcoding_config::*;
pub use visibility::*;

pub trait DatabaseTable {
	/// The name of the table in the database.
	const NAME: &'static str;

	/// The friendly name of the table. This is used in error messages and
	/// should be able to be pluralized. For example, "recording" or "playback
	/// session" as we can say "recording not found" or "playback sessions not
	/// found".
	const FRIENDLY_NAME: &'static str;
}
