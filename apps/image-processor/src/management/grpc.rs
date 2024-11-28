use scuffle_image_processor_proto::{CancelTaskRequest, CancelTaskResponse, ProcessImageRequest, ProcessImageResponse};
use tonic::{Request, Response};

use super::ManagementServer;

impl ManagementServer {
	#[tracing::instrument(skip_all)]
	pub async fn run_grpc(&self, addr: std::net::SocketAddr) -> Result<(), tonic::transport::Error> {
		let server = tonic::transport::Server::builder()
			.add_service(
				scuffle_image_processor_proto::image_processor_server::ImageProcessorServer::new(self.clone())
					.max_decoding_message_size(128 * 1024 * 1024)
					.max_encoding_message_size(128 * 1024 * 1024),
			)
			.serve_with_shutdown(addr, scuffle_context::Context::global().into_done());

		tracing::info!("gRPC management server listening on {}", addr);

		server.await
	}
}

#[async_trait::async_trait]
impl scuffle_image_processor_proto::image_processor_server::ImageProcessor for ManagementServer {
	#[tracing::instrument(skip_all)]
	async fn process_image(&self, request: Request<ProcessImageRequest>) -> tonic::Result<Response<ProcessImageResponse>> {
		let resp = match self.process_image(request.into_inner()).await {
			Ok(resp) => resp,
			Err(err) => ProcessImageResponse {
				id: "".to_owned(),
				upload_info: None,
				error: Some(err),
			},
		};

		Ok(Response::new(resp))
	}

	#[tracing::instrument(skip_all)]
	async fn cancel_task(&self, request: Request<CancelTaskRequest>) -> tonic::Result<Response<CancelTaskResponse>> {
		let resp = match self.cancel_task(request.into_inner()).await {
			Ok(resp) => resp,
			Err(err) => CancelTaskResponse { error: Some(err) },
		};

		Ok(Response::new(resp))
	}
}
