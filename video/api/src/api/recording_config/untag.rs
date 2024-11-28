use pb::scuffle::video::v1::events_fetch_request::Target;
use pb::scuffle::video::v1::types::access_token_scope::Permission;
use pb::scuffle::video::v1::types::{event, Resource};
use pb::scuffle::video::v1::{RecordingConfigUntagRequest, RecordingConfigUntagResponse};

use crate::api::utils::impl_request_scopes;
use crate::api::utils::tags::impl_untag_req;
use crate::ratelimit::RateLimitResource;

impl_request_scopes!(
	RecordingConfigUntagRequest,
	video_common::database::RecordingConfig,
	(Resource::RecordingConfig, Permission::Modify),
	RateLimitResource::RecordingConfigUntag
);

impl_untag_req!(RecordingConfigUntagRequest, RecordingConfigUntagResponse, Target::RecordingConfig, [id] {
	event::Event::RecordingConfig(event::RecordingConfig {
		recording_config_id: Some(id.into()),
		event: Some(event::recording_config::Event::Modified(event::recording_config::Modified {})),
	})
});
