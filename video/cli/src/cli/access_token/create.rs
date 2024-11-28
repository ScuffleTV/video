use anyhow::Context;
use pb::scuffle::video::v1::types::Tags;
use pb::scuffle::video::v1::AccessTokenCreateRequest;
use video_api::api::{RequiredScope, ResourcePermission};

use super::AccessToken;
use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;

#[derive(Debug, clap::Args)]
pub struct Create {
	/// The scopes for the access token
	#[clap(long, value_delimiter = ' ', required = true, num_args = 1..)]
	scopes: Vec<String>,

	/// The time at which the access token expires
	#[clap(long)]
	expires_at: Option<chrono::DateTime<chrono::Utc>>,

	/// Time to live for the access token in seconds
	#[clap(long, conflicts_with = "expires_at")]
	ttl: Option<i64>,

	/// The tags for the access token (JSON)
	#[clap(long, default_value = "{}")]
	tags: String,
}

impl Invokable for Create {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		let resp = invoker
			.invoke(AccessTokenCreateRequest {
				scopes: RequiredScope::from(
					self.scopes
						.iter()
						.map(|s| {
							s.parse::<ResourcePermission>()
								.map_err(|()| anyhow::anyhow!("failed to convert {s} into a resouce permission"))
						})
						.collect::<Result<Vec<_>, _>>()?,
				)
				.optimize()
				.0,
				tags: Some(Tags {
					tags: serde_json::from_str(&self.tags).context("failed to parse tags")?,
				}),
				expires_at: self
					.ttl
					.map(|ttl| chrono::Utc::now() + chrono::Duration::seconds(ttl))
					.or(self.expires_at)
					.map(|dt| dt.timestamp()),
			})
			.await?;

		invoker.display(&AccessToken::from(resp))?;

		Ok(())
	}
}

impl From<pb::scuffle::video::v1::AccessTokenCreateResponse> for AccessToken {
	fn from(value: pb::scuffle::video::v1::AccessTokenCreateResponse) -> Self {
		AccessToken::from_proto(value.access_token.unwrap_or_default(), Some(value.secret))
	}
}
