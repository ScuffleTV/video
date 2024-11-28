use std::path::PathBuf;

use bytes::Bytes;

use super::{Drive, DriveError, DriveWriteOptions};
use crate::config::{DriveMode, LocalDriveConfig};

#[derive(Debug)]
pub struct LocalDrive {
	name: String,
	mode: DriveMode,
	path: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum LocalDriveError {
	#[error("io: {0}")]
	Io(#[from] std::io::Error),
}

impl LocalDrive {
	#[tracing::instrument(skip(config), name = "LocalDisk::new", fields(name = %config.name), err)]
	pub async fn new(config: &LocalDriveConfig) -> Result<Self, DriveError> {
		tracing::debug!("setting up local disk");

		if !config.path.exists() {
			tokio::fs::create_dir_all(&config.path).await.map_err(LocalDriveError::Io)?;
		}

		Ok(Self {
			name: config.name.clone(),
			mode: config.mode,
			path: config.path.clone(),
		})
	}
}

impl Drive for LocalDrive {
	fn name(&self) -> &str {
		&self.name
	}

	#[tracing::instrument(skip(self), name = "LocalDisk::read", err)]
	async fn read(&self, path: &str) -> Result<Bytes, DriveError> {
		tracing::debug!("reading file");

		if self.mode == DriveMode::Write {
			return Err(DriveError::ReadOnly);
		}

		let path = self.path.join(path);
		Ok(tokio::fs::read(path).await.map_err(LocalDriveError::Io)?.into())
	}

	#[tracing::instrument(skip(self, data), name = "LocalDisk::write", err, fields(size = data.len()))]
	async fn write(&self, path: &str, data: Bytes, options: Option<DriveWriteOptions>) -> Result<(), DriveError> {
		tracing::debug!("writing file");

		if self.mode == DriveMode::Read {
			return Err(DriveError::WriteOnly);
		}

		let path = self.path.join(path);
		Ok(tokio::fs::write(path, data).await.map_err(LocalDriveError::Io)?)
	}

	#[tracing::instrument(skip(self), name = "LocalDisk::delete", err)]
	async fn delete(&self, path: &str) -> Result<(), DriveError> {
		tracing::debug!("deleting file");

		if self.mode == DriveMode::Read {
			return Err(DriveError::WriteOnly);
		}

		let path = self.path.join(path);
		tokio::fs::remove_file(path).await.map_err(LocalDriveError::Io)?;
		Ok(())
	}
}
