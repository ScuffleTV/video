use pb::scuffle::video::v1::events_fetch_request::Target;
use pb::scuffle::video::v1::types::access_token_scope::Permission;
use pb::scuffle::video::v1::types::{event, Resource};
use pb::scuffle::video::v1::{RoomUntagRequest, RoomUntagResponse};

use crate::api::utils::impl_request_scopes;
use crate::api::utils::tags::impl_untag_req;
use crate::ratelimit::RateLimitResource;

impl_request_scopes!(
	RoomUntagRequest,
	video_common::database::Room,
	(Resource::Room, Permission::Modify),
	RateLimitResource::RoomUntag
);

impl_untag_req!(RoomUntagRequest, RoomUntagResponse, Target::Room, [id] {
	event::Event::Room(event::Room {
		room_id: Some(id.into()),
		event: Some(event::room::Event::Modified(event::room::Modified {})),
	})
});
