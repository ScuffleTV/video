use pb::scuffle::video::v1::events_fetch_request::Target;
use pb::scuffle::video::v1::types::access_token_scope::Permission;
use pb::scuffle::video::v1::types::{event, Resource};
use pb::scuffle::video::v1::{RoomTagRequest, RoomTagResponse};

use crate::api::utils::impl_request_scopes;
use crate::api::utils::tags::impl_tag_req;
use crate::ratelimit::RateLimitResource;

impl_request_scopes!(
	RoomTagRequest,
	video_common::database::Room,
	(Resource::Room, Permission::Modify),
	RateLimitResource::RoomTag
);

impl_tag_req!(RoomTagRequest, RoomTagResponse, Target::Room, [id] {
	event::Event::Room(event::Room {
		room_id: Some(id.into()),
		event: Some(event::room::Event::Modified(event::room::Modified {})),
	})
});
