use std::sync::Arc;

use bytes::Bytes;
use common::http::ext::ResultExt;
use common::http::RouteError;
use common::make_response;
use hyper::{Body, Response, StatusCode};
use pb::scuffle::platform::internal::image_processor;
use pb::scuffle::platform::internal::types::{uploaded_file_metadata, ImageFormat, UploadedFileMetadata};
use serde_json::json;
use ulid::Ulid;

use super::UploadType;
use crate::api::auth::AuthData;
use crate::api::error::ApiError;
use crate::config::{ApiConfig, ImageUploaderConfig};
use crate::database::{FileType, RolePermission};
use crate::global::ApiGlobal;

fn create_task(task_id: Ulid, input_path: &str, config: &ImageUploaderConfig, owner_id: Ulid) -> image_processor::Task {
	image_processor::Task {
		input_path: input_path.to_string(),
		base_height: 128, // 128, 256, 384, 512
		base_width: 128,  // 128, 256, 384, 512
		formats: vec![
			ImageFormat::PngStatic as i32,
			ImageFormat::AvifStatic as i32,
			ImageFormat::WebpStatic as i32,
			ImageFormat::Gif as i32,
			ImageFormat::Webp as i32,
			ImageFormat::Avif as i32,
		],
		callback_subject: config.profile_picture_callback_subject.clone(),
		limits: Some(image_processor::task::Limits {
			max_input_duration_ms: 10 * 1000, // 10 seconds
			max_input_frame_count: 300,
			max_input_height: 1000,
			max_input_width: 1000,
			max_processing_time_ms: 60 * 1000, // 60 seconds
		}),
		resize_algorithm: image_processor::task::ResizeAlgorithm::Lanczos4 as i32,
		upscale: true, // For profile pictures we want to have a consistent size
		scales: vec![1, 2, 3, 4],
		resize_method: image_processor::task::ResizeMethod::PadCenter as i32,
		output_prefix: format!("{owner_id}/{task_id}"),
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum AcceptedFormats {
	Jpeg,
	Png,
	Webp,
	Gif,
	Avif,
}

impl AcceptedFormats {
	pub fn from_content_type(content_type: &str) -> Option<Self> {
		match content_type {
			"image/jpeg" => Some(Self::Jpeg),
			"image/png" => Some(Self::Png),
			"image/webp" => Some(Self::Webp),
			"image/gif" => Some(Self::Gif),
			"image/avif" => Some(Self::Avif),
			_ => None,
		}
	}

	pub const fn ext(self) -> &'static str {
		match self {
			Self::Jpeg => "jpg",
			Self::Png => "png",
			Self::Webp => "webp",
			Self::Gif => "gif",
			Self::Avif => "avif",
		}
	}
}

#[derive(Default, serde::Deserialize)]
#[serde(default)]
pub(super) struct ProfilePicture {
	set_active: bool,
}

impl UploadType for ProfilePicture {
	fn validate_format<G: ApiGlobal>(_: &Arc<G>, _: &AuthData, content_type: &str) -> bool {
		AcceptedFormats::from_content_type(content_type).is_some()
	}

	fn validate_permissions(&self, auth: &AuthData) -> bool {
		auth.user_permissions.has_permission(RolePermission::UploadProfilePicture)
	}

	fn get_max_size<G: ApiGlobal>(global: &Arc<G>) -> usize {
		global.config::<ApiConfig>().max_profile_picture_size
	}

	async fn handle<G: ApiGlobal>(
		self,
		global: &Arc<G>,
		auth: AuthData,
		name: Option<String>,
		file: Bytes,
		content_type: &str,
	) -> Result<Response<Body>, RouteError<ApiError>> {
		let image_format = AcceptedFormats::from_content_type(content_type)
			.ok_or((StatusCode::BAD_REQUEST, "invalid content-type header"))?;

		let task_id = Ulid::new();

		let config = global.config::<ImageUploaderConfig>();

		let input_path = format!(
			"{}/profile_pictures/{}/source.{}",
			auth.session.user_id,
			task_id,
			image_format.ext()
		);

		let mut s3_bucket = global.image_uploader_s3().clone();

		// The source image should be private
		s3_bucket.add_header("x-amz-acl", "private");

		let mut tx = global
			.db()
			.begin()
			.await
			.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to begin transaction"))?;

		sqlx::query("INSERT INTO image_jobs (id, priority, task) VALUES ($1, $2, $3)")
			.bind(common::database::Ulid(task_id))
			.bind(config.profile_picture_task_priority)
			.bind(common::database::Protobuf(create_task(
				task_id,
				&input_path,
				config,
				auth.session.user_id.0,
			)))
			.execute(tx.as_mut())
			.await
			.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to insert image job"))?;

		sqlx::query("INSERT INTO uploaded_files(id, owner_id, uploader_id, name, type, metadata, total_size, pending, path) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)")
            .bind(common::database::Ulid(task_id)) // id
            .bind(auth.session.user_id) // owner_id
            .bind(auth.session.user_id) // uploader_id
            .bind(name.unwrap_or_else(|| format!("untitled.{}", image_format.ext()))) // name
            .bind(FileType::ProfilePicture) // type
            .bind(common::database::Protobuf(UploadedFileMetadata {
				metadata: Some(uploaded_file_metadata::Metadata::Image(uploaded_file_metadata::Image {
					versions: Vec::new(),
				})),
			})) // metadata
            .bind(file.len() as i64) // total_size
            .bind(true) // pending
            .bind(&input_path) // path
            .execute(tx.as_mut())
            .await
            .map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to insert uploaded file"))?;

		if self.set_active {
			sqlx::query("UPDATE users SET pending_profile_picture_id = $1 WHERE id = $2")
				.bind(common::database::Ulid(task_id))
				.bind(auth.session.user_id)
				.execute(tx.as_mut())
				.await
				.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to update user"))?;
		}

		s3_bucket
			.put_object_with_content_type(&input_path, &file, content_type)
			.await
			.map_err(|err| {
				tracing::error!(error = %err, "failed to upload image to s3");
				(StatusCode::INTERNAL_SERVER_ERROR, "failed to upload image to s3")
			})?;

		tx.commit()
			.await
			.map_err(|err| {
				tracing::warn!(path = %input_path, "possible leaked s3 upload");
				err
			})
			.map_err_route((StatusCode::INTERNAL_SERVER_ERROR, "failed to commit transaction"))?;

		Ok(make_response!(
			StatusCode::OK,
			json!({
				"task_id": task_id.to_string(),
			})
		))
	}
}
