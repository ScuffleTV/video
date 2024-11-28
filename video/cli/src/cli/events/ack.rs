use pb::scuffle::video::v1::EventsAckRequest;
use ulid::Ulid;

use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;

#[derive(Debug, clap::Args)]
pub struct Ack {
	/// The id of the events to acknowledge
	#[clap(long, required = true)]
	id: Ulid,

	/// The action to take on the event
	#[clap(long, default_value = "ack")]
	action: Action,

	/// The delay to requeue the event with (milliseconds)
	#[clap(long, default_value = "0")]
	requeue_delay: u32,
}

#[derive(clap::ValueEnum, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
	Ack,
	Reject,
	Requeue,
	Reclaim,
}

impl std::fmt::Display for Action {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Action::Ack => write!(f, "ack"),
			Action::Reject => write!(f, "reject"),
			Action::Requeue => write!(f, "requeue"),
			Action::Reclaim => write!(f, "reclaim"),
		}
	}
}

#[derive(Debug, serde::Serialize)]
pub struct EventAck {
	id: Ulid,
	action: String,
}

impl Invokable for Ack {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		invoker
			.invoke(EventsAckRequest {
				id: Some(self.id.into()),
				action: Some(match self.action {
					Action::Ack => pb::scuffle::video::v1::events_ack_request::Action::Ack(true),
					Action::Reject => pb::scuffle::video::v1::events_ack_request::Action::Reject(true),
					Action::Requeue => {
						pb::scuffle::video::v1::events_ack_request::Action::RequeueDelayMs(self.requeue_delay)
					}
					Action::Reclaim => pb::scuffle::video::v1::events_ack_request::Action::Reclaim(true),
				}),
			})
			.await?;

		invoker.display(&EventAck {
			action: self.action.to_string(),
			id: self.id,
		})?;

		Ok(())
	}
}
