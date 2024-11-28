use pb::ext::UlidExt;
use ulid::Ulid;

use crate::cli::display::DeleteResponseFailed;
use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;

#[derive(Debug, clap::Args)]
pub struct ResetKey {
	/// The ids of the rooms to reset the key for
	#[clap(long, value_parser, num_args = 1.., value_delimiter = ' ', required = true)]
	ids: Vec<Ulid>,
}

#[derive(Debug, serde::Serialize)]
struct RoomResetKey {
	resets: Vec<KeyReset>,
	failed: Vec<DeleteResponseFailed>,
}

#[derive(Debug, serde::Serialize)]
struct KeyReset {
	id: Ulid,
	key: String,
}

impl Invokable for ResetKey {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		if self.ids.is_empty() {
			anyhow::bail!("no ids provided");
		}

		let resp = invoker
			.invoke(pb::scuffle::video::v1::RoomResetKeyRequest {
				ids: self.ids.iter().copied().map(|id| id.into()).collect(),
			})
			.await?;

		invoker.display(&RoomResetKey {
			failed: resp
				.failed_resets
				.into_iter()
				.map(|f| DeleteResponseFailed {
					id: f.id.into_ulid(),
					error: f.reason,
				})
				.collect(),
			resets: resp
				.rooms
				.into_iter()
				.map(|r| KeyReset {
					id: r.id.into_ulid(),
					key: r.key,
				})
				.collect(),
		})?;

		Ok(())
	}
}
