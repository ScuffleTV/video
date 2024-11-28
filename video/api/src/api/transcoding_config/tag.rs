use pb::scuffle::video::v1::events_fetch_request::Target;
use pb::scuffle::video::v1::types::access_token_scope::Permission;
use pb::scuffle::video::v1::types::{event, Resource};
use pb::scuffle::video::v1::{TranscodingConfigTagRequest, TranscodingConfigTagResponse};

use crate::api::utils::impl_request_scopes;
use crate::api::utils::tags::impl_tag_req;
use crate::ratelimit::RateLimitResource;

impl_request_scopes!(
	TranscodingConfigTagRequest,
	video_common::database::TranscodingConfig,
	(Resource::TranscodingConfig, Permission::Modify),
	RateLimitResource::TranscodingConfigTag
);

impl_tag_req!(TranscodingConfigTagRequest, TranscodingConfigTagResponse, Target::TranscodingConfig, [id] {
	event::Event::TranscodingConfig(event::TranscodingConfig {
		transcoding_config_id: Some(id.into()),
		event: Some(event::transcoding_config::Event::Modified(event::transcoding_config::Modified {})),
	})
});
