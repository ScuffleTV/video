use ulid::Ulid;

use crate::cli::display::DeleteResponse;
use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;
#[derive(Debug, clap::Args)]
pub struct Disconnect {
	/// The ids of the rooms to disconnect
	#[clap(long, value_parser, num_args = 1.., value_delimiter = ' ', required = true)]
	ids: Vec<Ulid>,
}

impl Invokable for Disconnect {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		let resp = invoker
			.invoke(pb::scuffle::video::v1::RoomDisconnectRequest {
				ids: self.ids.iter().copied().map(Into::into).collect(),
			})
			.await?;

		invoker.display(&DeleteResponse::from(resp))?;

		Ok(())
	}
}

impl From<pb::scuffle::video::v1::RoomDisconnectResponse> for DeleteResponse {
	fn from(resp: pb::scuffle::video::v1::RoomDisconnectResponse) -> Self {
		Self {
			ids: resp.ids.into_iter().map(|i| i.into_ulid()).collect(),
			failed: resp.failed_disconnects.into_iter().map(Into::into).collect(),
		}
	}
}
