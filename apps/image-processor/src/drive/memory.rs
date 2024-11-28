use std::collections::HashMap;

use bytes::Bytes;
use tokio::sync::RwLock;

use super::{Drive, DriveError, DriveWriteOptions};
use crate::config::{DriveMode, MemoryDriveConfig};

#[derive(Debug)]
struct FileHolder {
	remaining_capacity: usize,
	files: HashMap<String, MemoryFile>,
}

impl FileHolder {
	fn get(&self, path: &str) -> Option<&MemoryFile> {
		self.files.get(path)
	}

	fn insert(&mut self, path: String, file: MemoryFile) -> Result<Option<MemoryFile>, DriveError> {
		if file.data.len() > self.remaining_capacity {
			return Err(MemoryDriveError::NoSpaceLeft.into());
		}

		self.remaining_capacity -= file.data.len();
		self.files
			.insert(path, file)
			.map(|file| {
				self.remaining_capacity += file.data.len();
				Ok(file)
			})
			.transpose()
	}

	fn remove(&mut self, path: &str) -> Option<MemoryFile> {
		let file = self.files.remove(path)?;
		self.remaining_capacity += file.data.len();
		Some(file)
	}
}

#[derive(Debug)]
pub struct MemoryDrive {
	name: String,
	mode: DriveMode,
	files: RwLock<FileHolder>,
	acl: Option<String>,
}

#[derive(Debug, Clone)]
pub struct MemoryFile {
	data: Bytes,
	_options: DriveWriteOptions,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum MemoryDriveError {
	#[error("no space left on disk")]
	NoSpaceLeft,
}

impl MemoryDrive {
	#[tracing::instrument(skip(config), name = "MemoryDisk::new", fields(name = %config.name), err)]
	pub async fn new(config: &MemoryDriveConfig) -> Result<Self, MemoryDriveError> {
		tracing::debug!("setting up memory disk");
		Ok(Self {
			name: config.name.clone(),
			mode: config.mode,
			acl: config.acl.clone(),
			files: RwLock::new(FileHolder {
				remaining_capacity: config.capacity.unwrap_or(usize::MAX),
				files: HashMap::new(),
			}),
		})
	}
}

impl Drive for MemoryDrive {
	fn name(&self) -> &str {
		&self.name
	}

	#[tracing::instrument(skip(self), name = "MemoryDisk::read", err)]
	async fn read(&self, path: &str) -> Result<Bytes, DriveError> {
		tracing::debug!("reading file");

		if self.mode == DriveMode::Write {
			return Err(DriveError::ReadOnly);
		}

		self.files
			.read()
			.await
			.get(path)
			.map(|file| file.data.clone())
			.ok_or(DriveError::NotFound)
	}

	#[tracing::instrument(skip(self, data), name = "MemoryDisk::write", err, fields(size = data.len()))]
	async fn write(&self, path: &str, data: Bytes, options: Option<DriveWriteOptions>) -> Result<(), DriveError> {
		tracing::debug!("writing file");

		if self.mode == DriveMode::Read {
			return Err(DriveError::WriteOnly);
		}

		let mut files = self.files.write().await;

		let mut options = options.unwrap_or_default();

		options.acl = options.acl.or_else(|| self.acl.clone());

		files.insert(path.to_owned(), MemoryFile { data, _options: options })?;

		Ok(())
	}

	#[tracing::instrument(skip(self), name = "MemoryDisk::delete", err)]
	async fn delete(&self, path: &str) -> Result<(), DriveError> {
		tracing::debug!("deleting file");

		if self.mode == DriveMode::Read {
			return Err(DriveError::WriteOnly);
		}

		self.files.write().await.remove(path).ok_or(DriveError::NotFound)?;
		Ok(())
	}

	fn default_acl(&self) -> Option<&str> {
		self.acl.as_deref()
	}
}
