use std::sync::Arc;
use std::time::Duration;

use fred::interfaces::KeysInterface;
use futures_util::Future;
use scuffle_utils::prelude::FutureTimeout;
use scuffle_utilsratelimiter::{RateLimitResponse, RateLimiterOptions};
use tonic::metadata::AsciiMetadataValue;
use tonic::{Response, Status};
use ulid::Ulid;

use super::RequiredScope;
use crate::config::ApiConfig;
use crate::global::ApiGlobal;
use crate::ratelimit::RateLimitResource;

const RATELIMIT_BANNED_HEADER: &str = "x-ratelimit-banned";
const RATELIMIT_RESET_HEADER: &str = "x-ratelimit-reset";
const RATELIMIT_REMAINING_HEADER: &str = "x-ratelimit-remaining";

pub async fn ratelimit_scoped<G: ApiGlobal, T, F: Future<Output = tonic::Result<Response<T>>>>(
	global: &Arc<G>,
	organization_id: Ulid,
	resource: RateLimitResource,
	scoped_fn: impl FnOnce() -> F,
) -> tonic::Result<Response<T>> {
	let config = global.config::<ApiConfig>();

	let ratelimit_rules = config
		.rate_limit_rules
		.rules
		.get(&resource)
		.unwrap_or(&config.rate_limit_rules.default);

	if ratelimit_rules.cost == 0 {
		return scoped_fn().await;
	}

	if ratelimit_rules.cost > ratelimit_rules.quota || ratelimit_rules.quota == 0 {
		return Err(Status::resource_exhausted("resource disabled"));
	}

	let options = RateLimiterOptions {
		cost: ratelimit_rules.cost,
		quota: ratelimit_rules.quota,
		quota_reset_seconds: ratelimit_rules.quota_reset_seconds,
		exceeded_limit: config.rate_limit_rules.banned_exceeded.exceeded_limit,
		exceeded_reset_seconds: config.rate_limit_rules.banned_exceeded.exceeded_reset_seconds,
		banned_reset_seconds: config.rate_limit_rules.banned_exceeded.banned_reset_seconds,

		limit_key: format!("{resource}"),
		banned_key: "banned".to_string(),
		exceeded_key: "exceeded".to_string(),
		namespace: format!("{{ratelimit:organization:{organization_id}}}"),
	};

	let Ok(resp) = ratelimit(global, &options).timeout(Duration::from_secs(1)).await else {
		return Err(Status::internal("failed to rate limit"));
	};

	let mut resp = resp?;

	match scoped_fn().timeout(Duration::from_secs(5)).await {
		Ok(Ok(mut v)) => {
			if let Some(reset) = resp.reset {
				v.metadata_mut().insert(RATELIMIT_RESET_HEADER, reset.as_secs().into());
			}

			v.metadata_mut().insert(RATELIMIT_REMAINING_HEADER, resp.remaining.into());

			Ok(v)
		}
		Ok(Err(mut err)) => {
			if let Some(reset) = resp.reset {
				err.metadata_mut().insert(RATELIMIT_RESET_HEADER, reset.as_secs().into());
			}

			let restore_amount = ratelimit_rules.cost.saturating_sub(ratelimit_rules.failed_cost) as i64;

			if restore_amount > 0 {
				let redis = global.redis();
				resp.remaining = ratelimit_rules.quota as i64
					- redis
						.decr_by(format!("{}{}", options.namespace, options.limit_key), restore_amount)
						.await
						.unwrap_or(0);
			}

			err.metadata_mut().insert(RATELIMIT_REMAINING_HEADER, resp.remaining.into());

			Err(err)
		}
		Err(_) => {
			let mut status = Status::internal("failed to process request");

			if let Some(reset) = resp.reset {
				status.metadata_mut().insert(RATELIMIT_RESET_HEADER, reset.as_secs().into());
			}

			status
				.metadata_mut()
				.insert(RATELIMIT_REMAINING_HEADER, resp.remaining.into());

			Err(status)
		}
	}
}

async fn ratelimit<G: ApiGlobal>(global: &Arc<G>, options: &RateLimiterOptions) -> tonic::Result<RateLimitResponse> {
	let redis = global.redis();

	let resp = scuffle_utilsratelimiter::ratelimit(redis.as_ref(), options)
		.await
		.map_err(|err| {
			tracing::error!(err = %err, "failed to rate limit");
			Status::internal("Unable to process request, failed to rate limit")
		})?;

	if resp.banned || resp.remaining == -1 {
		let mut status = Status::resource_exhausted("rate limit exceeded");

		if let Some(reset) = resp.reset {
			status.metadata_mut().insert(RATELIMIT_RESET_HEADER, reset.as_secs().into());
		}

		if resp.banned {
			status
				.metadata_mut()
				.insert(RATELIMIT_BANNED_HEADER, AsciiMetadataValue::from_static("true"));
		}

		status
			.metadata_mut()
			.insert(RATELIMIT_REMAINING_HEADER, AsciiMetadataValue::from_static("0"));

		Err(status)
	} else {
		Ok(resp)
	}
}

pub trait TonicRequest {
	type Table;

	fn permission_scope(&self) -> RequiredScope;
	fn ratelimit_scope(&self) -> RateLimitResource;
}

macro_rules! scope_ratelimit {
	($self:ident, $request:ident, $global:ident, $access_token:ident, $logic:expr) => {
		let $global = $request
			.extensions()
			.get::<std::sync::Arc<G>>()
			.ok_or_else(|| tonic::Status::internal("global state missing"))?;

		let $access_token = crate::api::utils::auth::validate_request(
			&$request,
			&crate::api::utils::TonicRequest::permission_scope($request.get_ref()),
		)?;

		return crate::api::utils::ratelimit::ratelimit_scoped(
			&$global,
			$access_token.organization_id,
			crate::api::utils::TonicRequest::ratelimit_scope($request.get_ref()),
			$logic,
		)
		.await;
	};
}

pub(crate) use scope_ratelimit;
