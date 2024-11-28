use anyhow::Context;
use pb::scuffle::video::v1::types::{SearchOptions, Tags};
use pb::scuffle::video::v1::RecordingGetRequest;
use ulid::Ulid;

use super::{Recording, Visibility};
use crate::cli::{Cli, Invokable};
use crate::invoker::Invoker;

#[derive(Debug, clap::Args)]
pub struct Get {
	/// The ids of the recordings to get
	#[clap(long, value_parser, num_args = 1.., value_delimiter = ' ')]
	ids: Vec<Ulid>,

	/// Filter by the room id of the recording
	#[clap(long)]
	room_id: Option<Ulid>,

	/// Filter by the recording config id of the recording
	#[clap(long)]
	recording_config_id: Option<Ulid>,

	/// Filter by the s3 bucket id of the recording
	#[clap(long)]
	s3_bucket_id: Option<Ulid>,

	/// Filter by the visibility of the recording
	#[clap(long)]
	visibility: Option<Visibility>,

	/// Filter by the deleted status of the recording
	#[clap(long)]
	deleted: Option<bool>,

	/// The maximum number of recordings to get
	#[clap(long, default_value = "100")]
	limit: usize,

	/// The ID after which to start getting recordings
	#[clap(long)]
	after: Option<Ulid>,

	/// The tags to filter recordings by (JSON)
	#[clap(long, default_value = "{}")]
	tags: String,

	/// Reverse the order of the recordings
	#[clap(long)]
	reverse: bool,
}

impl Invokable for Get {
	async fn invoke(&self, invoker: &mut Invoker, _: &Cli) -> anyhow::Result<()> {
		let resp = invoker
			.invoke(RecordingGetRequest {
				ids: self.ids.iter().copied().map(Into::into).collect(),
				room_id: self.room_id.map(Into::into),
				recording_config_id: self.recording_config_id.map(Into::into),
				s3_bucket_id: self.s3_bucket_id.map(Into::into),
				visibility: self.visibility.map(|v| match v {
					Visibility::Public => pb::scuffle::video::v1::types::Visibility::Public as i32,
					Visibility::Private => pb::scuffle::video::v1::types::Visibility::Private as i32,
				}),
				deleted: self.deleted,
				search_options: Some(SearchOptions {
					limit: self.limit as i32,
					after_id: self.after.map(Into::into),
					tags: Some(Tags {
						tags: serde_json::from_str(&self.tags).context("failed to parse tags")?,
					}),
					reverse: self.reverse,
				}),
			})
			.await?;

		invoker.display_array(&resp.recordings.into_iter().map(Recording::from_proto).collect::<Vec<_>>())?;

		Ok(())
	}
}
