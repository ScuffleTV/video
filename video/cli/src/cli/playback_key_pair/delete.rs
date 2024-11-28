use pb::scuffle::video::v1::PlaybackKeyPairDeleteRequest;
use ulid::Ulid;

use crate::cli::display::DeleteResponse;
use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;

#[derive(Debug, clap::Args)]
pub struct Delete {
	/// The ids of the playback key pairs to delete
	#[clap(long, value_parser, num_args = 1.., value_delimiter = ' ', required = true)]
	ids: Vec<Ulid>,
}

impl Invokable for Delete {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		if self.ids.is_empty() {
			return Err(anyhow::anyhow!("must specify at least one id"));
		}

		let resp = invoker
			.invoke(PlaybackKeyPairDeleteRequest {
				ids: self.ids.iter().copied().map(Into::into).collect(),
			})
			.await?;

		invoker.display(&DeleteResponse::from(resp))?;

		Ok(())
	}
}

impl From<pb::scuffle::video::v1::PlaybackKeyPairDeleteResponse> for DeleteResponse {
	fn from(resp: pb::scuffle::video::v1::PlaybackKeyPairDeleteResponse) -> Self {
		Self {
			ids: resp.ids.into_iter().map(|i| i.into_ulid()).collect(),
			failed: resp.failed_deletes.into_iter().map(Into::into).collect(),
		}
	}
}
