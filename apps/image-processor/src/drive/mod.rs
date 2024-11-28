use bytes::Bytes;

use self::http::{HttpDrive, HttpDriveError};
use self::local::{LocalDrive, LocalDriveError};
use self::memory::{MemoryDrive, MemoryDriveError};
use self::public_http::{PublicHttpDrive, PublicHttpDriveError};
use self::s3::{S3Drive, S3DriveError};
use crate::config::DriveConfig;

pub mod http;
pub mod local;
pub mod memory;
pub mod public_http;
pub mod s3;

#[derive(Debug, thiserror::Error)]
pub enum DriveError {
	#[error("http: {0}")]
	Http(#[from] HttpDriveError),
	#[error("local: {0}")]
	Local(#[from] LocalDriveError),
	#[error("s3: {0}")]
	S3(#[from] S3DriveError),
	#[error("memory: {0}")]
	Memory(#[from] MemoryDriveError),
	#[error("public http: {0}")]
	PublicHttp(#[from] PublicHttpDriveError),
	#[error("not found")]
	NotFound,
	#[error("read only")]
	ReadOnly,
	#[error("write only")]
	WriteOnly,
}

#[derive(Debug, Clone, Default)]
pub struct DriveWriteOptions {
	pub cache_control: Option<String>,
	pub content_type: Option<String>,
	pub acl: Option<String>,
	pub content_disposition: Option<String>,
}

pub trait Drive {
	/// Get the name of the drive
	fn name(&self) -> &str;

	/// Read data from a drive
	fn read(&self, path: &str) -> impl std::future::Future<Output = Result<Bytes, DriveError>> + Send;

	/// Write data to a drive
	fn write(
		&self,
		path: &str,
		data: Bytes,
		options: Option<DriveWriteOptions>,
	) -> impl std::future::Future<Output = Result<(), DriveError>> + Send;

	/// Delete data from a drive
	fn delete(&self, path: &str) -> impl std::future::Future<Output = Result<(), DriveError>> + Send;

	fn healthy(&self) -> impl std::future::Future<Output = bool> + Send {
		async { true }
	}

	fn default_acl(&self) -> Option<&str> {
		None
	}
}

#[derive(Debug)]
pub enum AnyDrive {
	Local(LocalDrive),
	S3(S3Drive),
	Memory(MemoryDrive),
	Http(HttpDrive),
	PublicHttp(PublicHttpDrive),
}

impl Drive for AnyDrive {
	fn name(&self) -> &str {
		match self {
			AnyDrive::Local(drive) => drive.name(),
			AnyDrive::S3(drive) => drive.name(),
			AnyDrive::Memory(drive) => drive.name(),
			AnyDrive::Http(drive) => drive.name(),
			AnyDrive::PublicHttp(drive) => drive.name(),
		}
	}

	async fn read(&self, path: &str) -> Result<Bytes, DriveError> {
		match self {
			AnyDrive::Local(drive) => drive.read(path).await,
			AnyDrive::S3(drive) => drive.read(path).await,
			AnyDrive::Memory(drvie) => drvie.read(path).await,
			AnyDrive::Http(drive) => drive.read(path).await,
			AnyDrive::PublicHttp(drive) => drive.read(path).await,
		}
	}

	async fn write(&self, path: &str, data: Bytes, options: Option<DriveWriteOptions>) -> Result<(), DriveError> {
		match self {
			AnyDrive::Local(drive) => drive.write(path, data, options).await,
			AnyDrive::S3(drive) => drive.write(path, data, options).await,
			AnyDrive::Memory(drive) => drive.write(path, data, options).await,
			AnyDrive::Http(drive) => drive.write(path, data, options).await,
			AnyDrive::PublicHttp(drive) => drive.write(path, data, options).await,
		}
	}

	async fn delete(&self, path: &str) -> Result<(), DriveError> {
		match self {
			AnyDrive::Local(drive) => drive.delete(path).await,
			AnyDrive::S3(drive) => drive.delete(path).await,
			AnyDrive::Memory(drive) => drive.delete(path).await,
			AnyDrive::Http(drive) => drive.delete(path).await,
			AnyDrive::PublicHttp(drive) => drive.delete(path).await,
		}
	}
}

pub async fn build_drive(config: &DriveConfig) -> Result<AnyDrive, DriveError> {
	match config {
		DriveConfig::Local(local) => Ok(AnyDrive::Local(LocalDrive::new(local).await?)),
		DriveConfig::S3(s3) => Ok(AnyDrive::S3(S3Drive::new(s3).await?)),
		DriveConfig::Memory(memory) => Ok(AnyDrive::Memory(MemoryDrive::new(memory).await?)),
		DriveConfig::Http(http) => Ok(AnyDrive::Http(HttpDrive::new(http).await?)),
		DriveConfig::PublicHttp(public_http) => Ok(AnyDrive::PublicHttp(PublicHttpDrive::new(public_http).await?)),
	}
}
