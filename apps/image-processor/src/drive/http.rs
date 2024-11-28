use bytes::Bytes;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::Method;
use url::Url;

use super::{Drive, DriveError, DriveWriteOptions};
use crate::config::{DriveMode, HttpDriveConfig};

#[derive(Debug)]
pub struct HttpDrive {
	name: String,
	base_url: Url,
	mode: DriveMode,
	semaphore: Option<tokio::sync::Semaphore>,
	client: reqwest::Client,
	acl: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum HttpDriveError {
	#[error("invalid path")]
	InvalidPath(#[from] url::ParseError),
	#[error("reqwest: {0}")]
	Reqwest(#[from] reqwest::Error),
	#[error("invalid header name")]
	InvalidHeaderName(#[from] reqwest::header::InvalidHeaderName),
	#[error("invalid header value")]
	InvalidHeaderValue(#[from] reqwest::header::InvalidHeaderValue),
}

impl HttpDrive {
	#[tracing::instrument(skip(config), name = "HttpDisk::new", fields(name = %config.name), err)]
	pub async fn new(config: &HttpDriveConfig) -> Result<Self, DriveError> {
		tracing::debug!("setting up http disk");
		Ok(Self {
			name: config.name.clone(),
			base_url: config.url.clone(),
			mode: config.mode,
			semaphore: config.max_connections.map(|max| tokio::sync::Semaphore::new(max)),
			client: {
				let mut builder = reqwest::Client::builder();

				if let Some(timeout) = config.timeout {
					builder = builder.timeout(timeout);
				}

				if config.allow_insecure {
					builder = builder.danger_accept_invalid_certs(true);
				}

				let mut headers = HeaderMap::new();

				for (key, value) in &config.headers {
					headers.insert(
						key.parse::<HeaderName>().map_err(HttpDriveError::InvalidHeaderName)?,
						value.parse::<HeaderValue>().map_err(HttpDriveError::InvalidHeaderValue)?,
					);
				}

				builder = builder.default_headers(headers);

				builder.build().map_err(HttpDriveError::Reqwest)?
			},
			acl: config.acl.clone(),
		})
	}
}

impl Drive for HttpDrive {
	fn name(&self) -> &str {
		&self.name
	}

	#[tracing::instrument(skip(self), name = "HttpDisk::read", fields(name = %self.name), err)]
	async fn read(&self, path: &str) -> Result<Bytes, DriveError> {
		tracing::debug!("reading file");

		if self.mode == DriveMode::Write {
			return Err(DriveError::ReadOnly);
		}

		let _permit = if let Some(semaphore) = &self.semaphore {
			Some(semaphore.acquire().await)
		} else {
			None
		};

		let url = self.base_url.join(path).map_err(HttpDriveError::InvalidPath)?;

		let response = self.client.get(url).send().await.map_err(HttpDriveError::Reqwest)?;

		let response = response.error_for_status().map_err(HttpDriveError::Reqwest)?;

		Ok(response.bytes().await.map_err(HttpDriveError::Reqwest)?)
	}

	#[tracing::instrument(skip(self, data), name = "HttpDisk::write", fields(name = %self.name, size = data.len()), err)]
	async fn write(&self, path: &str, data: Bytes, options: Option<DriveWriteOptions>) -> Result<(), DriveError> {
		tracing::debug!("writing file");

		if self.mode == DriveMode::Read {
			return Err(DriveError::WriteOnly);
		}

		let _permit = if let Some(semaphore) = &self.semaphore {
			Some(semaphore.acquire().await)
		} else {
			None
		};

		let url = self.base_url.join(path).map_err(HttpDriveError::InvalidPath)?;

		let mut request = self
			.client
			.request(Method::POST, url)
			.body(data)
			.build()
			.map_err(HttpDriveError::Reqwest)?;

		if let Some(options) = options {
			if let Some(cache_control) = &options.cache_control {
				request.headers_mut().insert(
					reqwest::header::CACHE_CONTROL,
					cache_control.parse().map_err(HttpDriveError::InvalidHeaderValue)?,
				);
			}

			if let Some(content_type) = &options.content_type {
				request.headers_mut().insert(
					reqwest::header::CONTENT_TYPE,
					content_type.parse().map_err(HttpDriveError::InvalidHeaderValue)?,
				);
			}

			if let Some(acl) = options.acl.as_ref().or(self.acl.as_ref()) {
				request.headers_mut().insert(
					reqwest::header::HeaderName::from_static("x-amz-acl"),
					acl.parse().map_err(HttpDriveError::InvalidHeaderValue)?,
				);
			}
		}

		let resp = self.client.execute(request).await.map_err(HttpDriveError::Reqwest)?;

		resp.error_for_status().map_err(HttpDriveError::Reqwest)?;

		Ok(())
	}

	#[tracing::instrument(skip(self), name = "HttpDisk::delete", fields(name = %self.name), err)]
	async fn delete(&self, path: &str) -> Result<(), DriveError> {
		tracing::debug!("deleting file");

		if self.mode == DriveMode::Read {
			return Err(DriveError::WriteOnly);
		}

		let _permit = if let Some(semaphore) = &self.semaphore {
			Some(semaphore.acquire().await)
		} else {
			None
		};

		let url = self.base_url.join(path).map_err(HttpDriveError::InvalidPath)?;

		let response = self.client.delete(url).send().await.map_err(HttpDriveError::Reqwest)?;

		response.error_for_status().map_err(HttpDriveError::Reqwest)?;

		Ok(())
	}

	fn default_acl(&self) -> Option<&str> {
		self.acl.as_deref()
	}
}
