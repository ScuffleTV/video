use anyhow::Context as _;
use base64::Engine;
use futures_util::stream::BoxStream;
pub use pb::scuffle::video::v1::*;
use scuffle_utils::context::Context;
use tonic::service::interceptor;
use tonic::transport::Channel;
use ulid::Ulid;

use crate::cli::display::{DeleteResponse, TagResponse};
pub use crate::invoker::request::*;

type AuthChannel = interceptor::InterceptedService<Channel, AuthInterceptor>;

pub struct GrpcBackend {
	_channel: Channel,
	access_token_client: pb::scuffle::video::v1::access_token_client::AccessTokenClient<AuthChannel>,
	events_client: pb::scuffle::video::v1::events_client::EventsClient<AuthChannel>,
	playback_key_pair_client: pb::scuffle::video::v1::playback_key_pair_client::PlaybackKeyPairClient<AuthChannel>,
	playback_session_client: pb::scuffle::video::v1::playback_session_client::PlaybackSessionClient<AuthChannel>,
	recording_client: pb::scuffle::video::v1::recording_client::RecordingClient<AuthChannel>,
	room_client: pb::scuffle::video::v1::room_client::RoomClient<AuthChannel>,
	s3_bucket_client: pb::scuffle::video::v1::s3_bucket_client::S3BucketClient<AuthChannel>,
	transcoding_config_client: pb::scuffle::video::v1::transcoding_config_client::TranscodingConfigClient<AuthChannel>,
	recording_config_client: pb::scuffle::video::v1::recording_config_client::RecordingConfigClient<AuthChannel>,
	_context: Context,
}

#[derive(Clone, Copy)]
struct AuthInterceptor {
	organization_id: Ulid,
	access_key: Ulid,
	secret_key: Ulid,
}

impl tonic::service::Interceptor for AuthInterceptor {
	fn call(&mut self, mut request: tonic::Request<()>) -> tonic::Result<tonic::Request<()>> {
		request
			.metadata_mut()
			.insert("x-scuffle-organization-id", self.organization_id.to_string().parse().unwrap());

		let auth =
			base64::engine::general_purpose::URL_SAFE.encode(format!("{}:{}", self.access_key, self.secret_key).as_bytes());

		request
			.metadata_mut()
			.insert("authorization", format!("Basic {auth}").parse().unwrap());
		Ok(request)
	}
}

impl GrpcBackend {
	pub async fn new(
		context: Context,
		access_key: &str,
		secret_key: &str,
		endpoint: &str,
		organization_id: Ulid,
	) -> anyhow::Result<Self> {
		let channel = Channel::from_shared(endpoint.to_string())?
			.connect()
			.await
			.context("failed to connect to gRPC endpoint")?;

		let interceptor = AuthInterceptor {
			organization_id,
			access_key: access_key.parse().context("failed to parse access key")?,
			secret_key: secret_key.parse().context("failed to parse secret key")?,
		};

		let access_token_client =
			pb::scuffle::video::v1::access_token_client::AccessTokenClient::with_interceptor(channel.clone(), interceptor);
		let events_client =
			pb::scuffle::video::v1::events_client::EventsClient::with_interceptor(channel.clone(), interceptor);
		let playback_key_pair_client =
			pb::scuffle::video::v1::playback_key_pair_client::PlaybackKeyPairClient::with_interceptor(
				channel.clone(),
				interceptor,
			);
		let playback_session_client =
			pb::scuffle::video::v1::playback_session_client::PlaybackSessionClient::with_interceptor(
				channel.clone(),
				interceptor,
			);
		let recording_client =
			pb::scuffle::video::v1::recording_client::RecordingClient::with_interceptor(channel.clone(), interceptor);
		let room_client = pb::scuffle::video::v1::room_client::RoomClient::with_interceptor(channel.clone(), interceptor);
		let s3_bucket_client =
			pb::scuffle::video::v1::s3_bucket_client::S3BucketClient::with_interceptor(channel.clone(), interceptor);
		let transcoding_config_client =
			pb::scuffle::video::v1::transcoding_config_client::TranscodingConfigClient::with_interceptor(
				channel.clone(),
				interceptor,
			);
		let recording_config_client =
			pb::scuffle::video::v1::recording_config_client::RecordingConfigClient::with_interceptor(
				channel.clone(),
				interceptor,
			);

		Ok(Self {
			_channel: channel,
			access_token_client,
			events_client,
			playback_key_pair_client,
			playback_session_client,
			recording_client,
			room_client,
			s3_bucket_client,
			transcoding_config_client,
			recording_config_client,
			_context: context,
		})
	}
}

impl_request!(
	GrpcBackend;

	|self, req: AccessTokenCreateRequest| -> AccessTokenCreateResponse {
		Ok(self.access_token_client.create(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: AccessTokenDeleteRequest| -> AccessTokenDeleteResponse {
		Ok(self.access_token_client.delete(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: AccessTokenGetRequest| -> AccessTokenGetResponse {
		Ok(self.access_token_client.get(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: AccessTokenTagRequest| -> AccessTokenTagResponse {
		Ok(self.access_token_client.tag(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: AccessTokenUntagRequest| -> AccessTokenUntagResponse {
		Ok(self.access_token_client.untag(req).await.context("failed call grpc endpoint")?.into_inner())
	},

	|self, req: EventsFetchRequest| -> BoxStream<'static, tonic::Result<EventsFetchResponse>> {
		Ok(Box::pin(self.events_client.fetch(req).await.context("failed call grpc endpoint")?.into_inner()))
	},
	|self, req: EventsAckRequest| -> EventsAckResponse {
		Ok(self.events_client.ack(req).await.context("failed call grpc endpoint")?.into_inner())
	},

	|self, _req: OrganizationCreateRequest| -> Organization {
		anyhow::bail!("gRPC backend does not support organization create")
	},
	|self, _req: OrganizationDeleteRequest| -> DeleteResponse {
		anyhow::bail!("gRPC backend does not support organization delete")
	},
	|self, _req: OrganizationGetRequest| -> Vec<Organization> {
		anyhow::bail!("gRPC backend does not support organization get")
	},
	|self, _req: OrganizationModifyRequest| -> Organization {
		anyhow::bail!("gRPC backend does not support organization modify")
	},
	|self, _req: OrganizationTagRequest| -> TagResponse {
		anyhow::bail!("gRPC backend does not support organization tag")
	},
	|self, _req: OrganizationUntagRequest| -> TagResponse {
		anyhow::bail!("gRPC backend does not support organization untag")
	},

	|self, req: PlaybackKeyPairCreateRequest| -> PlaybackKeyPairCreateResponse {
		Ok(self.playback_key_pair_client.create(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: PlaybackKeyPairDeleteRequest| -> PlaybackKeyPairDeleteResponse {
		Ok(self.playback_key_pair_client.delete(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: PlaybackKeyPairGetRequest| -> PlaybackKeyPairGetResponse {
		Ok(self.playback_key_pair_client.get(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: PlaybackKeyPairTagRequest| -> PlaybackKeyPairTagResponse {
		Ok(self.playback_key_pair_client.tag(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: PlaybackKeyPairUntagRequest| -> PlaybackKeyPairUntagResponse {
		Ok(self.playback_key_pair_client.untag(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: PlaybackKeyPairModifyRequest| -> PlaybackKeyPairModifyResponse {
		Ok(self.playback_key_pair_client.modify(req).await.context("failed call grpc endpoint")?.into_inner())
	},

	|self, req: PlaybackSessionCountRequest| -> PlaybackSessionCountResponse {
		Ok(self.playback_session_client.count(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: PlaybackSessionGetRequest| -> PlaybackSessionGetResponse {
		Ok(self.playback_session_client.get(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: PlaybackSessionRevokeRequest| -> PlaybackSessionRevokeResponse {
		Ok(self.playback_session_client.revoke(req).await.context("failed call grpc endpoint")?.into_inner())
	},

	|self, req: RecordingDeleteRequest| -> RecordingDeleteResponse {
		Ok(self.recording_client.delete(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: RecordingGetRequest| -> RecordingGetResponse {
		Ok(self.recording_client.get(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: RecordingModifyRequest| -> RecordingModifyResponse {
		Ok(self.recording_client.modify(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: RecordingTagRequest| -> RecordingTagResponse {
		Ok(self.recording_client.tag(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: RecordingUntagRequest| -> RecordingUntagResponse {
		Ok(self.recording_client.untag(req).await.context("failed call grpc endpoint")?.into_inner())
	},

	|self, req: RecordingConfigCreateRequest| -> RecordingConfigCreateResponse {
		Ok(self.recording_config_client.create(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: RecordingConfigDeleteRequest| -> RecordingConfigDeleteResponse {
		Ok(self.recording_config_client.delete(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: RecordingConfigGetRequest| -> RecordingConfigGetResponse {
		Ok(self.recording_config_client.get(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: RecordingConfigModifyRequest| -> RecordingConfigModifyResponse {
		Ok(self.recording_config_client.modify(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: RecordingConfigTagRequest| -> RecordingConfigTagResponse {
		Ok(self.recording_config_client.tag(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: RecordingConfigUntagRequest| -> RecordingConfigUntagResponse {
		Ok(self.recording_config_client.untag(req).await.context("failed call grpc endpoint")?.into_inner())
	},

	|self, req: RoomCreateRequest| -> RoomCreateResponse {
		Ok(self.room_client.create(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: RoomDeleteRequest| -> RoomDeleteResponse {
		Ok(self.room_client.delete(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: RoomGetRequest| -> RoomGetResponse {
		Ok(self.room_client.get(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: RoomModifyRequest| -> RoomModifyResponse {
		Ok(self.room_client.modify(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: RoomTagRequest| -> RoomTagResponse {
		Ok(self.room_client.tag(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: RoomUntagRequest| -> RoomUntagResponse {
		Ok(self.room_client.untag(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: RoomDisconnectRequest| -> RoomDisconnectResponse {
		Ok(self.room_client.disconnect(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: RoomResetKeyRequest| -> RoomResetKeyResponse {
		Ok(self.room_client.reset_key(req).await.context("failed call grpc endpoint")?.into_inner())
	},

	|self, req: S3BucketCreateRequest| -> S3BucketCreateResponse {
		Ok(self.s3_bucket_client.create(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: S3BucketDeleteRequest| -> S3BucketDeleteResponse {
		Ok(self.s3_bucket_client.delete(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: S3BucketGetRequest| -> S3BucketGetResponse {
		Ok(self.s3_bucket_client.get(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: S3BucketModifyRequest| -> S3BucketModifyResponse {
		Ok(self.s3_bucket_client.modify(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: S3BucketTagRequest| -> S3BucketTagResponse {
		Ok(self.s3_bucket_client.tag(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: S3BucketUntagRequest| -> S3BucketUntagResponse {
		Ok(self.s3_bucket_client.untag(req).await.context("failed call grpc endpoint")?.into_inner())
	},

	|self, req: TranscodingConfigCreateRequest| -> TranscodingConfigCreateResponse {
		Ok(self.transcoding_config_client.create(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: TranscodingConfigDeleteRequest| -> TranscodingConfigDeleteResponse {
		Ok(self.transcoding_config_client.delete(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: TranscodingConfigGetRequest| -> TranscodingConfigGetResponse {
		Ok(self.transcoding_config_client.get(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: TranscodingConfigModifyRequest| -> TranscodingConfigModifyResponse {
		Ok(self.transcoding_config_client.modify(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: TranscodingConfigTagRequest| -> TranscodingConfigTagResponse {
		Ok(self.transcoding_config_client.tag(req).await.context("failed call grpc endpoint")?.into_inner())
	},
	|self, req: TranscodingConfigUntagRequest| -> TranscodingConfigUntagResponse {
		Ok(self.transcoding_config_client.untag(req).await.context("failed call grpc endpoint")?.into_inner())
	},
);
