use bytes::Bytes;
use http::{HeaderName, HeaderValue};

use super::{Drive, DriveError, DriveWriteOptions};
use crate::config::PublicHttpDriveConfig;

pub const PUBLIC_HTTP_DRIVE_NAME: &str = "__public_http";

#[derive(Debug)]
pub struct PublicHttpDrive {
	client: reqwest::Client,
	semaphore: Option<tokio::sync::Semaphore>,
}

#[derive(Debug, thiserror::Error)]
pub enum PublicHttpDriveError {
	#[error("reqwest: {0}")]
	Reqwest(#[from] reqwest::Error),
	#[error("invalid header name")]
	InvalidHeaderName(#[from] reqwest::header::InvalidHeaderName),
	#[error("invalid header value")]
	InvalidHeaderValue(#[from] reqwest::header::InvalidHeaderValue),
	#[error("unsupported: {0}")]
	Unsupported(&'static str),
}

impl PublicHttpDrive {
	#[tracing::instrument(skip(config), name = "PublicHttpDisk::new", err)]
	pub async fn new(config: &PublicHttpDriveConfig) -> Result<Self, DriveError> {
		tracing::debug!("setting up public http disk");
		if !config.blacklist.is_empty() || !config.whitelist.is_empty() {
			tracing::error!("blacklist and whitelist are not currently implemented for public http disk");
			return Err(PublicHttpDriveError::Unsupported("blacklist and whitelist").into());
		}

		Ok(Self {
			client: {
				let mut builder = reqwest::Client::builder();

				if let Some(timeout) = config.timeout {
					builder = builder.timeout(timeout);
				}

				if config.allow_insecure {
					builder = builder.danger_accept_invalid_certs(true);
				}

				let mut headers = reqwest::header::HeaderMap::new();

				for (key, value) in &config.headers {
					headers.insert(
						key.parse::<HeaderName>().map_err(PublicHttpDriveError::from)?,
						value.parse::<HeaderValue>().map_err(PublicHttpDriveError::from)?,
					);
				}

				builder = builder.default_headers(headers);

				builder.build().map_err(PublicHttpDriveError::Reqwest)?
			},
			semaphore: config.max_connections.map(|max| tokio::sync::Semaphore::new(max)),
		})
	}
}

impl Drive for PublicHttpDrive {
	fn name(&self) -> &str {
		PUBLIC_HTTP_DRIVE_NAME
	}

	#[tracing::instrument(skip(self), name = "PublicHttpDisk::read", err)]
	async fn read(&self, path: &str) -> Result<Bytes, DriveError> {
		tracing::debug!("reading file");

		let _permit = if let Some(semaphore) = &self.semaphore {
			Some(semaphore.acquire().await)
		} else {
			None
		};

		let response = self.client.get(path).send().await.map_err(PublicHttpDriveError::Reqwest)?;

		let response = response.error_for_status().map_err(PublicHttpDriveError::Reqwest)?;

		Ok(response.bytes().await.map_err(PublicHttpDriveError::Reqwest)?)
	}

	#[tracing::instrument(skip(self, data), name = "PublicHttpDisk::write", fields(size = data.len()), err)]
	async fn write(&self, path: &str, data: Bytes, options: Option<DriveWriteOptions>) -> Result<(), DriveError> {
		tracing::error!("writing is not supported for public http disk");
		Err(DriveError::ReadOnly)
	}

	#[tracing::instrument(skip(self), name = "PublicHttpDisk::delete", err)]
	async fn delete(&self, path: &str) -> Result<(), DriveError> {
		tracing::error!("deleting is not supported for public http disk");
		Err(DriveError::ReadOnly)
	}
}
