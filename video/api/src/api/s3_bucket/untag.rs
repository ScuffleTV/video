use pb::scuffle::video::v1::events_fetch_request::Target;
use pb::scuffle::video::v1::types::access_token_scope::Permission;
use pb::scuffle::video::v1::types::{event, Resource};
use pb::scuffle::video::v1::{S3BucketUntagRequest, S3BucketUntagResponse};

use crate::api::utils::impl_request_scopes;
use crate::api::utils::tags::impl_untag_req;
use crate::ratelimit::RateLimitResource;

impl_request_scopes!(
	S3BucketUntagRequest,
	video_common::database::S3Bucket,
	(Resource::S3Bucket, Permission::Modify),
	RateLimitResource::S3BucketUntag
);

impl_untag_req!(S3BucketUntagRequest, S3BucketUntagResponse, Target::S3Bucket, [id] {
	event::Event::S3Bucket(event::S3Bucket {
		s3_bucket_id: Some(id.into()),
		event: Some(event::s3_bucket::Event::Modified(event::s3_bucket::Modified {})),
	})
});
