pub mod access_tokens;
pub mod auth;
pub mod get;
pub mod ratelimit;
pub mod tags;

use std::sync::Arc;

pub use access_tokens::{AccessTokenExt, RequiredScope, ResourcePermission};
pub use ratelimit::TonicRequest;
use video_common::database::AccessToken;

use crate::global::ApiGlobal;

macro_rules! impl_request_scopes {
	($type:ty, $table:ty, $permission:expr, $ratelimit:expr) => {
		impl crate::api::utils::TonicRequest for $type {
			type Table = $table;

			#[inline(always)]
			fn permission_scope(&self) -> crate::api::utils::RequiredScope {
				crate::api::utils::ResourcePermission::from($permission).into()
			}

			#[inline(always)]
			fn ratelimit_scope(&self) -> crate::ratelimit::RateLimitResource {
				($ratelimit).into()
			}
		}
	};
}

pub(crate) use impl_request_scopes;

#[allow(async_fn_in_trait)]
pub trait ApiRequest<R> {
	async fn process<G: ApiGlobal>(&self, global: &Arc<G>, access_token: &AccessToken) -> tonic::Result<tonic::Response<R>>;
}
