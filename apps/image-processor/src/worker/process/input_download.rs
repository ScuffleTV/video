use std::sync::Arc;

use bson::oid::ObjectId;
use bytes::Bytes;
use scuffle_image_processor_proto::{input, Input};

use crate::drive::Drive;
use crate::global::Global;

#[derive(Debug, thiserror::Error)]
pub enum InputDownloadError {
	#[error("missing public http drive")]
	MissingPublicHttpDrive,
	#[error("missing drive")]
	MissingDrive,
	#[error("missing input")]
	MissingInput,
	#[error("drive error: {0}")]
	DriveError(#[from] crate::drive::DriveError),
	#[error("strfmt error: {0}")]
	StrFmtError(#[from] strfmt::FmtError),
}

fn get_path(input: Option<&Input>) -> Option<&str> {
	match input?.path.as_ref()? {
		input::Path::DrivePath(drive) => Some(&drive.path),
		input::Path::PublicUrl(url) => Some(url),
	}
}

fn get_drive(input: Option<&Input>) -> Option<&str> {
	match input?.path.as_ref()? {
		input::Path::DrivePath(drive) => Some(&drive.drive),
		input::Path::PublicUrl(_) => None,
	}
}

#[tracing::instrument(skip(global, input), fields(input_path = get_path(input), input_drive = get_drive(input)))]
pub async fn download_input(global: &Arc<Global>, id: ObjectId, input: Option<&Input>) -> Result<Bytes, InputDownloadError> {
	match input
		.ok_or(InputDownloadError::MissingInput)?
		.path
		.as_ref()
		.ok_or(InputDownloadError::MissingInput)?
	{
		input::Path::DrivePath(drive) => {
			let path = strfmt::strfmt(&drive.path, &([("id".to_owned(), id.to_string())].into_iter().collect()))?;

			Ok(global
				.drive(&drive.drive)
				.ok_or(InputDownloadError::MissingDrive)?
				.read(&path)
				.await?)
		}
		input::Path::PublicUrl(url) => Ok(global
			.public_http_drive()
			.ok_or(InputDownloadError::MissingPublicHttpDrive)?
			.read(url)
			.await?),
	}
}
