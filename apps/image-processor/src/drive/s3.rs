use aws_config::{AppName, Region};
use aws_sdk_s3::config::{Credentials, SharedCredentialsProvider};
use aws_sdk_s3::operation::delete_object::DeleteObjectError;
use aws_sdk_s3::operation::get_object::GetObjectError;
use aws_sdk_s3::operation::put_object::PutObjectError;
use aws_smithy_runtime_api::client::orchestrator::HttpResponse;
use aws_smithy_runtime_api::client::result::SdkError;
use bytes::Bytes;

use super::{Drive, DriveError, DriveWriteOptions};
use crate::config::{DriveMode, S3DriveConfig};

#[derive(Debug)]
pub struct S3Drive {
	name: String,
	mode: DriveMode,
	client: aws_sdk_s3::Client,
	bucket: String,
	path_prefix: Option<String>,
	semaphore: Option<tokio::sync::Semaphore>,
	acl: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum S3DriveError {
	#[error("s3: {0}")]
	S3(#[from] aws_sdk_s3::Error),
	#[error("byte stream: {0}")]
	ByteStream(#[from] aws_smithy_types::byte_stream::error::Error),
	#[error("read: {0}")]
	Read(#[from] SdkError<GetObjectError, HttpResponse>),
	#[error("write: {0}")]
	Write(#[from] SdkError<PutObjectError, HttpResponse>),
	#[error("delete: {0}")]
	Delete(#[from] SdkError<DeleteObjectError, HttpResponse>),
}

impl S3Drive {
	#[tracing::instrument(skip(config), name = "S3Disk::new", fields(name = %config.name), err)]
	pub async fn new(config: &S3DriveConfig) -> Result<Self, DriveError> {
		tracing::debug!("setting up s3 disk");
		Ok(Self {
			name: config.name.clone(),
			mode: config.mode,
			client: aws_sdk_s3::Client::from_conf({
				let mut builder = aws_sdk_s3::Config::builder();

				builder.set_endpoint_url(config.endpoint.clone());

				builder.set_app_name(Some(AppName::new("scuffle-image-processor").unwrap()));

				builder.set_region(Some(Region::new(config.region.clone())));

				builder.set_force_path_style(config.force_path_style);

				builder.set_credentials_provider(Some(SharedCredentialsProvider::new(Credentials::new(
					config.access_key.clone(),
					config.secret_key.clone(),
					None,
					None,
					"ConfiguredCredentialsProvider",
				))));

				builder.build()
			}),
			path_prefix: config.prefix_path.clone(),
			bucket: config.bucket.clone(),
			semaphore: config.max_connections.map(tokio::sync::Semaphore::new),
			acl: config.acl.clone(),
		})
	}
}

impl Drive for S3Drive {
	fn name(&self) -> &str {
		&self.name
	}

	#[tracing::instrument(skip(self), name = "S3Disk::read", err)]
	async fn read(&self, path: &str) -> Result<Bytes, DriveError> {
		if self.mode == DriveMode::Write {
			return Err(DriveError::ReadOnly);
		}

		let _permit = if let Some(semaphore) = &self.semaphore {
			Some(semaphore.acquire().await)
		} else {
			None
		};

		let path = self
			.path_prefix
			.as_ref()
			.map_or_else(|| path.to_string(), |prefix| format!("{}/{}", prefix, path));

		let result = self
			.client
			.get_object()
			.bucket(&self.bucket)
			.key(path.trim_start_matches('/'))
			.send()
			.await
			.map_err(S3DriveError::from)?;

		let bytes = result.body.collect().await.map_err(S3DriveError::from)?;

		Ok(bytes.into_bytes())
	}

	#[tracing::instrument(skip(self, data), name = "S3Disk::write", err, fields(size = data.len()))]
	async fn write(&self, path: &str, data: Bytes, options: Option<DriveWriteOptions>) -> Result<(), DriveError> {
		if self.mode == DriveMode::Read {
			return Err(DriveError::WriteOnly);
		}

		let _permit = if let Some(semaphore) = &self.semaphore {
			Some(semaphore.acquire().await)
		} else {
			None
		};

		let path = self
			.path_prefix
			.as_ref()
			.map_or_else(|| path.to_string(), |prefix| format!("{}/{}", prefix, path));

		let mut req = self
			.client
			.put_object()
			.bucket(&self.bucket)
			.key(path.trim_start_matches('/'))
			.body(data.into());

		let options = options.unwrap_or_default();

		if let Some(cache_control) = &options.cache_control {
			req = req.cache_control(cache_control);
		}
		if let Some(content_type) = &options.content_type {
			req = req.content_type(content_type);
		}
		if let Some(content_disposition) = &options.content_disposition {
			req = req.content_disposition(content_disposition);
		}
		if let Some(acl) = options.acl.as_ref().or(self.acl.as_ref()) {
			req = req.acl(acl.as_str().into());
		}

		req.send().await.map_err(S3DriveError::from).inspect_err(|err| {
			tracing::error!("failed to write to s3: {:?}", err);
		})?;

		Ok(())
	}

	#[tracing::instrument(skip(self), name = "S3Disk::delete", err)]
	async fn delete(&self, path: &str) -> Result<(), DriveError> {
		if self.mode == DriveMode::Read {
			return Err(DriveError::WriteOnly);
		}

		let _permit = if let Some(semaphore) = &self.semaphore {
			Some(semaphore.acquire().await)
		} else {
			None
		};

		let path = self
			.path_prefix
			.as_ref()
			.map_or_else(|| path.to_string(), |prefix| format!("{}/{}", prefix, path));

		self.client
			.delete_object()
			.bucket(&self.bucket)
			.key(path.trim_start_matches('/'))
			.send()
			.await
			.map_err(S3DriveError::from)?;

		Ok(())
	}

	fn default_acl(&self) -> Option<&str> {
		self.acl.as_deref()
	}
}
