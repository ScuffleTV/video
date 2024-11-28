use anyhow::Context;
use ulid::Ulid;

use crate::cli::{Cli, Invokable};
use crate::invoker::request::OrganizationGetRequest;
use crate::invoker::Invoker;

#[derive(clap::Args, Debug)]
pub struct Get {
	/// The ids of the organizations to get
	#[clap(long, value_parser, num_args = 1.., value_delimiter = ' ')]
	ids: Vec<Ulid>,

	/// The maximum number of organizations to get
	#[clap(long, default_value = "100")]
	limit: usize,

	/// The ID after which to start getting organizations
	#[clap(long)]
	after: Option<Ulid>,

	/// The tags to filter organizations by (JSON)
	#[clap(long, default_value = "{}")]
	tags: String,

	/// Reverse the order of the organizations
	#[clap(long)]
	reverse: bool,
}

impl Invokable for Get {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		let resp = invoker
			.invoke(OrganizationGetRequest {
				ids: self.ids.clone(),
				search_options: Some(pb::scuffle::video::v1::types::SearchOptions {
					limit: self.limit as _,
					after_id: self.after.map(|id| id.into()),
					tags: Some(pb::scuffle::video::v1::types::Tags {
						tags: serde_json::from_str(&self.tags).context("failed to parse tags")?,
					}),
					reverse: self.reverse,
				}),
			})
			.await?;

		invoker.display_array(&resp)?;

		Ok(())
	}
}
