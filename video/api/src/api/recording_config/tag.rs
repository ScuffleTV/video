use pb::scuffle::video::v1::events_fetch_request::Target;
use pb::scuffle::video::v1::types::access_token_scope::Permission;
use pb::scuffle::video::v1::types::{event, Resource};
use pb::scuffle::video::v1::{RecordingConfigTagRequest, RecordingConfigTagResponse};

use crate::api::utils::impl_request_scopes;
use crate::api::utils::tags::impl_tag_req;
use crate::ratelimit::RateLimitResource;

impl_request_scopes!(
	RecordingConfigTagRequest,
	video_common::database::RecordingConfig,
	(Resource::RecordingConfig, Permission::Modify),
	RateLimitResource::RecordingConfigTag
);

impl_tag_req!(RecordingConfigTagRequest, RecordingConfigTagResponse, Target::RecordingConfig, [id] {
	event::Event::RecordingConfig(event::RecordingConfig {
		recording_config_id: Some(id.into()),
		event: Some(event::recording_config::Event::Modified(event::recording_config::Modified {})),
	})
});
