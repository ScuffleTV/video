use pb::scuffle::video::v1::events_fetch_request::Target;
use pb::scuffle::video::v1::types::access_token_scope::Permission;
use pb::scuffle::video::v1::types::{event, Resource};
use pb::scuffle::video::v1::{AccessTokenUntagRequest, AccessTokenUntagResponse};

use crate::api::utils::impl_request_scopes;
use crate::api::utils::tags::impl_untag_req;
use crate::ratelimit::RateLimitResource;

impl_request_scopes!(
	AccessTokenUntagRequest,
	video_common::database::AccessToken,
	(Resource::AccessToken, Permission::Modify),
	RateLimitResource::AccessTokenUntag
);

impl_untag_req!(AccessTokenUntagRequest, AccessTokenUntagResponse, Target::AccessToken, [id] {
	event::Event::AccessToken(event::AccessToken {
		access_token_id: Some(id.into()),
		event: Some(event::access_token::Event::Modified(event::access_token::Modified {})),
	})
});
