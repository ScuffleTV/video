use std::sync::Arc;
use std::time::Duration;

use bson::Bson;
use mongodb::bson::oid::ObjectId;
use mongodb::options::IndexOptions;
use mongodb::{Database, IndexModel};
use scuffle_image_processor_proto::Task;

use crate::global::Global;

mod protobuf {
	use serde::{Deserialize, Serializer};

	pub fn serialize<T: prost::Message, S: Serializer>(value: &T, serializer: S) -> Result<S::Ok, S::Error> {
		serializer.serialize_bytes(&value.encode_to_vec())
	}

	pub fn deserialize<'de, T: prost::Message + Default, D: serde::Deserializer<'de>>(
		deserializer: D,
	) -> Result<T, D::Error> {
		let binary = bson::Binary::deserialize(deserializer)?;
		T::decode(binary.bytes.as_slice()).map_err(serde::de::Error::custom)
	}
}

mod datetime {
	use serde::{Deserialize, Serialize, Serializer};

	pub fn deserialize<'de, D: serde::Deserializer<'de>>(
		deserializer: D,
	) -> Result<Option<chrono::DateTime<chrono::Utc>>, D::Error> {
		let bson_datetime = Option::<bson::DateTime>::deserialize(deserializer)?;
		Ok(bson_datetime.map(|dt| dt.into()))
	}

	pub fn serialize<S: Serializer>(
		value: &Option<chrono::DateTime<chrono::Utc>>,
		serializer: S,
	) -> Result<S::Ok, S::Error> {
		match value {
			Some(value) => bson::DateTime::from(*value).serialize(serializer),
			None => None::<bson::DateTime>.serialize(serializer),
		}
	}
}

#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct Job {
	#[serde(rename = "_id")]
	/// The id of the job
	pub id: ObjectId,
	/// The priority of the job, higher priority jobs are fetched first
	pub priority: u32,
	/// The lease time of the job on a worker.
	#[serde(with = "datetime")]
	pub hold_until: Option<chrono::DateTime<chrono::Utc>>,
	#[serde(with = "protobuf")]
	/// The task to be performed
	pub task: Task,
	/// The ttl of the job, after which it will be deleted
	#[serde(with = "datetime")]
	pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
	/// The id of the worker that claimed the job
	pub claimed_by_id: Option<ObjectId>,
}

impl Job {
	fn collection(database: &Database) -> mongodb::Collection<Job> {
		database.collection("jobs")
	}

	pub async fn setup_collection(database: &Database) -> Result<(), mongodb::error::Error> {
		let collection = Self::collection(database);

		collection
			.create_index(
				IndexModel::builder()
					.keys(bson::doc! {
						"hold_until": 1,
						"priority": -1,
					})
					.build(),
			)
			.await?;

		collection
			.create_index(
				IndexModel::builder()
					.keys(bson::doc! {
						"expires_at": 1,
					})
					.options(Some(
						IndexOptions::builder().expire_after(Some(Duration::from_secs(0))).build(),
					))
					.build(),
			)
			.await?;

		Ok(())
	}

	/// Creates a new job in the database
	/// # Arguments
	/// * `global` - The global state
	/// * `task` - The task to be performed
	/// * `priority` - The priority of the job
	/// * `ttl` - The time-to-live of the job in seconds
	/// # Returns
	/// The job that was created
	pub async fn new(
		global: &Arc<Global>,
		id: ObjectId,
		task: Task,
		priority: u32,
		ttl: Option<u32>,
	) -> Result<Self, mongodb::error::Error> {
		let job = Job {
			id,
			priority,
			hold_until: None,
			task,
			claimed_by_id: None,
			expires_at: ttl.map(|ttl| chrono::Utc::now() + chrono::Duration::seconds(ttl as i64)),
		};

		Self::collection(global.database()).insert_one(&job).await?;

		Ok(job)
	}

	/// Fetches a job from the database
	/// The job is claimed by the worker and will be held for 60 seconds, after
	/// which it will be released to refresh the hold time, call `refresh`. The
	/// job returned is the one with the highest priority and no hold_until or
	/// hold_until in the past # Arguments
	/// * `global` - The global state
	/// # Returns
	/// The job that was fetched or None if no job was found
	pub async fn fetch(global: &Arc<Global>) -> Result<Option<Self>, mongodb::error::Error> {
		// Find with the highest priority and no hold_until or hold_until in the past
		Self::collection(global.database())
			.find_one_and_update(
				bson::doc! {
					"$or": [
						bson::doc! {
							"hold_until": Bson::Null,
						},
						bson::doc! {
							"hold_until": {
								"$lt": chrono::Utc::now(),
							},
						},
					],
				},
				bson::doc! {
					"$set": {
						"claimed_by_id": global.worker_id(),
						"hold_until": chrono::Utc::now() + global.config().worker.hold_time,
					},
				},
			)
			.with_options(
				mongodb::options::FindOneAndUpdateOptions::builder()
					.sort(bson::doc! {
						"priority": -1,
					})
					.build(),
			)
			.await
	}

	/// Refreshes the hold time of the job
	/// # Arguments
	/// * `global` - The global state
	/// # Returns
	/// Whether the job was successfully refreshed, if the job was reclaimed by
	/// a different worker, it will not be refreshed and this will return false
	pub async fn refresh(&self, global: &Arc<Global>) -> Result<bool, mongodb::error::Error> {
		let success = Self::collection(global.database())
			.update_one(
				bson::doc! {
					"_id": self.id,
					"claimed_by_id": global.worker_id(),
				},
				bson::doc! {
					"$set": {
						"hold_until": chrono::Utc::now() + global.config().worker.hold_time,
					},
				},
			)
			.await?;

		Ok(success.modified_count == 1)
	}

	/// Completes the job
	/// # Arguments
	/// * `global` - The global state
	/// # Returns
	/// Whether the job was successfully completed or not, if the job was
	/// reclaimed by a different worker, it will not be completed and this will
	/// return false
	pub async fn complete(&self, global: &Arc<Global>) -> Result<bool, mongodb::error::Error> {
		let success = Self::collection(global.database())
			.delete_one(bson::doc! {
				"_id": self.id,
				"claimed_by_id": global.worker_id(),
			})
			.await?;

		Ok(success.deleted_count == 1)
	}

	/// Cancels a job
	/// # Arguments
	/// * `global` - The global state
	/// * `id` - The id of the job to cancel
	/// # Returns
	/// The job that was cancelled or None if no job was found
	pub async fn cancel(global: &Arc<Global>, id: ObjectId) -> Result<Option<Job>, mongodb::error::Error> {
		let Some(job) = Self::collection(global.database())
			.find_one_and_delete(bson::doc! {
				"_id": id,
			})
			.await?
		else {
			return Ok(None);
		};

		// If the event had a cancel event, publish it
		crate::events::on_cancel(global, &job).await;

		Ok(Some(job))
	}
}
