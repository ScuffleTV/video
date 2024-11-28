use ulid::Ulid;

use crate::cli::display::DeleteResponse;
use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;

#[derive(Debug, clap::Args)]
pub struct Delete {
	/// The ids of the s3 buckets to delete
	#[clap(long, value_parser, num_args = 1.., value_delimiter = ' ', required = true)]
	ids: Vec<Ulid>,
}

impl Invokable for Delete {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		if self.ids.is_empty() {
			anyhow::bail!("no ids provided");
		}

		let resp = invoker
			.invoke(pb::scuffle::video::v1::S3BucketDeleteRequest {
				ids: self.ids.iter().copied().map(|id| id.into()).collect(),
			})
			.await?;

		invoker.display(&DeleteResponse::from(resp))?;

		Ok(())
	}
}

impl From<pb::scuffle::video::v1::S3BucketDeleteResponse> for DeleteResponse {
	fn from(resp: pb::scuffle::video::v1::S3BucketDeleteResponse) -> Self {
		Self {
			ids: resp.ids.into_iter().map(|id| id.into_ulid()).collect(),
			failed: resp.failed_deletes.into_iter().map(Into::into).collect(),
		}
	}
}
