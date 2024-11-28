use pb::scuffle::video::v1::events_fetch_request::Target;
use pb::scuffle::video::v1::types::access_token_scope::Permission;
use pb::scuffle::video::v1::types::{event, Resource};
use pb::scuffle::video::v1::{TranscodingConfigUntagRequest, TranscodingConfigUntagResponse};

use crate::api::utils::impl_request_scopes;
use crate::api::utils::tags::impl_untag_req;
use crate::ratelimit::RateLimitResource;

impl_request_scopes!(
	TranscodingConfigUntagRequest,
	video_common::database::TranscodingConfig,
	(Resource::TranscodingConfig, Permission::Modify),
	RateLimitResource::TranscodingConfigUntag
);

impl_untag_req!(TranscodingConfigUntagRequest, TranscodingConfigUntagResponse, Target::TranscodingConfig, [id] {
	event::Event::TranscodingConfig(event::TranscodingConfig {
		transcoding_config_id: Some(id.into()),
		event: Some(event::transcoding_config::Event::Modified(event::transcoding_config::Modified {})),
	})
});
