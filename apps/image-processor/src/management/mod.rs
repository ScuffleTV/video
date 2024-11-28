use std::sync::Arc;

use anyhow::Context;
use bson::oid::ObjectId;
use bytes::Bytes;
use scuffle_bootstrap::service::Service;
use scuffle_image_processor_proto::{
	input, CancelTaskRequest, CancelTaskResponse, DrivePath, Error, ErrorCode, Input, ProcessImageRequest,
	ProcessImageResponse, ProcessImageResponseUploadInfo,
};

use crate::database::Job;
use crate::drive::{Drive, DriveWriteOptions};
use crate::global::Global;
use crate::management::validation::{validate_input_upload, validate_task, FragmentBuf};
use crate::worker::process::DecoderFrontend;

pub mod grpc;
pub mod http;

mod utils;
mod validation;

#[derive(Clone)]
struct ManagementServer {
	global: Arc<Global>,
}

impl ManagementServer {
	#[tracing::instrument(skip_all)]
	async fn process_image(&self, mut request: ProcessImageRequest) -> Result<ProcessImageResponse, Error> {
		let mut fragment = FragmentBuf::new();

		tracing::info!("new process image request");

		validate_task(
			&self.global,
			fragment.push("task"),
			request.task.as_ref(),
			request.input_upload.as_ref().and_then(|upload| upload.drive_path.as_ref()),
		)?;

		// We need to do validation here.
		if let Some(input_upload) = request.input_upload.as_ref() {
			validate_input_upload(&self.global, fragment.push("input_upload"), Some(input_upload))?;
		}

		let id = ObjectId::new();

		let upload_info = if let Some(input_upload) = request.input_upload {
			let drive_path = input_upload.drive_path.unwrap();
			let drive = self.global.drive(&drive_path.drive).unwrap();

			let file_format = file_format::FileFormat::from_bytes(&input_upload.binary);

			DecoderFrontend::from_format(file_format).map_err(|err| Error {
				code: ErrorCode::Decode as i32,
				message: format!("input_upload.binary: {err}"),
			})?;

			let vars = [
				("id".to_owned(), id.to_string()),
				("ext".to_owned(), file_format.extension().to_owned()),
			]
			.into_iter()
			.collect();

			let path = strfmt::strfmt(&drive_path.path, &vars).map_err(|err| Error {
				code: ErrorCode::InvalidInput as i32,
				message: format!("input_upload.drive_path.path: {err}"),
			})?;

			let drive_path = DrivePath {
				drive: drive_path.drive,
				acl: drive_path.acl,
				path: path.clone(),
			};

			if let Some(input) = request.task.as_mut().unwrap().input.as_mut() {
				input.path = Some(input::Path::DrivePath(drive_path.clone()));
			} else {
				request.task.as_mut().unwrap().input = Some(Input {
					path: Some(input::Path::DrivePath(drive_path.clone())),
					..Default::default()
				});
			}

			let upload_size = input_upload.binary.len() as u64;

			drive
				.write(
					&path,
					Bytes::from(input_upload.binary),
					Some(DriveWriteOptions {
						acl: drive_path.acl.clone(),
						cache_control: input_upload.cache_control,
						content_disposition: input_upload.content_disposition,
						content_type: input_upload.content_type,
					}),
				)
				.await
				.map_err(|err| {
					tracing::error!("failed to write input upload: {:#}", err);
					Error {
						code: ErrorCode::Internal as i32,
						message: format!("failed to write input upload: {err}"),
					}
				})?;

			Some(ProcessImageResponseUploadInfo {
				path: Some(drive_path),
				content_type: file_format.media_type().to_owned(),
				size: upload_size,
			})
		} else {
			None
		};

		let job = Job::new(&self.global, id, request.task.unwrap(), request.priority, request.ttl)
			.await
			.map_err(|err| {
				tracing::error!("failed to create job: {:#}", err);
				Error {
					code: ErrorCode::Internal as i32,
					message: format!("failed to create job: {err}"),
				}
			})?;

		Ok(ProcessImageResponse {
			id: job.id.to_string(),
			upload_info,
			error: None,
		})
	}

	#[tracing::instrument(skip_all)]
	async fn cancel_task(&self, request: CancelTaskRequest) -> Result<CancelTaskResponse, Error> {
		tracing::info!("new cancel task request");

		match Job::cancel(
			&self.global,
			request.id.parse().map_err(|err| Error {
				code: ErrorCode::InvalidInput as i32,
				message: format!("id: {err}"),
			})?,
		)
		.await
		{
			Ok(Some(_)) => Ok(CancelTaskResponse { error: None }),
			Ok(None) => Err(Error {
				code: ErrorCode::InvalidInput as i32,
				message: "not found".to_owned(),
			}),
			Err(err) => {
				tracing::error!("failed to cancel job: {:#}", err);
				Err(Error {
					code: ErrorCode::Internal as i32,
					message: format!("failed to cancel job: {err}"),
				})
			}
		}
	}
}

pub struct ManagementSvc;

impl Service<Global> for ManagementSvc {
	async fn enabled(&self, global: &Arc<Global>) -> anyhow::Result<bool> {
		Ok(global.config().management.http.enabled || global.config().management.grpc.enabled)
	}

	async fn run(self, global: Arc<Global>, ctx: scuffle_context::Context) -> anyhow::Result<()> {
		let server = ManagementServer { global };

		let http = async {
			if server.global.config().management.http.enabled {
				let addr = utils::true_bind(server.global.config().management.http.bind).await?;
				server.run_http(addr).await.context("http")
			} else {
				std::future::pending().await
			}
		};
		let grpc = async {
			if server.global.config().management.grpc.enabled {
				let addr = utils::true_bind(server.global.config().management.grpc.bind).await?;
				server.run_grpc(addr).await.context("grpc")
			} else {
				std::future::pending().await
			}
		};

		tokio::select! {
			r = http => r.context("http")?,
			r = grpc => r.context("grpc")?,
			_ = ctx.done() => {},
		}

		tracing::info!("management shutdown");

		Ok(())
	}
}
