use std::sync::Arc;

use anyhow::Context;
use pb::ext::UlidExt;
use pb::scuffle::video::v1::types::{AudioConfig, Rendition, TranscodingConfig, VideoConfig};
use prost::Message;
use ulid::Ulid;
use video_common::database::Room;

use super::recording::Recording;
use crate::global::TranscoderGlobal;
use crate::transcoder::job::renditions::determine_output_renditions;

#[allow(dead_code)]
pub struct SqlOperations {
	pub transcoding_config: TranscodingConfig,
	pub recording: Option<Recording>,
	pub video_input: VideoConfig,
	pub audio_input: AudioConfig,
	pub video_output: Vec<VideoConfig>,
	pub audio_output: Vec<AudioConfig>,
}

pub async fn perform_sql_operations(
	global: &Arc<impl TranscoderGlobal>,
	organization_id: Ulid,
	room_id: Ulid,
	connection_id: Ulid,
) -> anyhow::Result<SqlOperations> {
	let mut client = global.db().get().await.context("failed to get database connection")?;

	let room: Option<Room> = match scuffle_utils::database::query(
		r#"
        SELECT 
            *
        FROM rooms
        WHERE
            organization_id = $1 AND
            id = $2 AND
            active_ingest_connection_id = $3
        "#,
	)
	.bind(organization_id)
	.bind(room_id)
	.bind(connection_id)
	.build_query_as()
	.fetch_optional(&client)
	.await
	{
		Ok(r) => r,
		Err(err) => {
			anyhow::bail!("failed to query room: {}", err);
		}
	};

	let Some(room) = room else {
		anyhow::bail!("room not found");
	};

	let Some(video_input) = room.video_input else {
		anyhow::bail!("room has no video input");
	};

	let Some(audio_input) = room.audio_input else {
		anyhow::bail!("room has no audio input");
	};

	let recording_config = if let Some(recording_config) = room.active_recording_config {
		Some(recording_config)
	} else if let Some(recording_config_id) = &room.recording_config_id {
		Some(
			match scuffle_utils::database::query(
				r#"
				SELECT
					*
				FROM
					recording_configs
				WHERE
					organization_id = $1
					AND id = $2
				"#,
			)
			.bind(organization_id)
			.bind(recording_config_id)
			.build_query_as::<video_common::database::RecordingConfig>()
			.fetch_one(&client)
			.await
			{
				Ok(r) => r.into_proto(),
				Err(err) => {
					anyhow::bail!("failed to query recording config: {}", err);
				}
			},
		)
	} else {
		None
	};

	let recording_config = if let Some(recording_config) = recording_config {
		let s3_bucket_id = recording_config.s3_bucket_id.into_ulid();

		Some((
			recording_config,
			match scuffle_utils::database::query(
				r#"
				SELECT
					*
				FROM
					s3_buckets
				WHERE
					organization_id = $1
					AND id = $2
				"#,
			)
			.bind(organization_id)
			.bind(s3_bucket_id)
			.build_query_as()
			.fetch_one(&client)
			.await
			{
				Ok(r) => r,
				Err(err) => {
					anyhow::bail!("failed to query s3 buckets: {}", err);
				}
			},
		))
	} else {
		None
	};

	let transcoding_config = if let Some(transcoding_config) = room.active_transcoding_config {
		transcoding_config
	} else if let Some(transcoding_config_id) = &room.transcoding_config_id {
		match scuffle_utils::database::query(
			r#"
			SELECT
				*
			FROM
				transcoding_configs
			WHERE
				organization_id = $1
				AND id = $2
			"#,
		)
		.bind(organization_id)
		.bind(transcoding_config_id)
		.build_query_as::<video_common::database::TranscodingConfig>()
		.fetch_one(&client)
		.await
		{
			Ok(r) => r.into_proto(),
			Err(err) => {
				anyhow::bail!("failed to query transcoding config: {}", err);
			}
		}
	} else {
		TranscodingConfig {
			renditions: vec![Rendition::AudioSource.into(), Rendition::VideoSource.into()],
			..Default::default()
		}
	};

	let (video_output, audio_output) = determine_output_renditions(&video_input, &audio_input, &transcoding_config);

	let tx = client.transaction().await.context("failed to start transaction")?;

	scuffle_utils::database::query(
		r#"
        UPDATE rooms
        SET
            updated_at = NOW(),
            active_transcoding_config = $1,
            active_recording_config = $2,
            video_output = $3,
            audio_output = $4
        WHERE 
            organization_id = $5 AND
            id = $6 AND
            active_ingest_connection_id = $7
    	"#,
	)
	.bind(transcoding_config.encode_to_vec())
	.bind(recording_config.as_ref().map(|(r, _)| r.encode_to_vec()))
	.bind(video_output.iter().map(|v| v.encode_to_vec()).collect::<Vec<_>>())
	.bind(audio_output.iter().map(|v| v.encode_to_vec()).collect::<Vec<_>>())
	.bind(organization_id)
	.bind(room_id)
	.bind(connection_id)
	.build()
	.execute(&tx)
	.await?;

	let recording = if let Some((recording_config, s3_bucket)) = &recording_config {
		Some(
			Recording::new(
				global,
				&tx,
				room.active_recording_id.map(Ulid::from).unwrap_or_else(Ulid::new),
				organization_id,
				room_id,
				room.visibility,
				&audio_output,
				&video_output,
				s3_bucket,
				recording_config,
			)
			.await?,
		)
	} else {
		None
	};

	tx.commit().await?;

	Ok(SqlOperations {
		transcoding_config,
		recording,
		video_input,
		audio_input,
		video_output,
		audio_output,
	})
}
