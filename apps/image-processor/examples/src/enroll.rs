use anyhow::Context;
use scuffle_image_processor_proto::image_processor_client::ImageProcessorClient;
use scuffle_image_processor_proto::output::Resize;
use scuffle_image_processor_proto::{
	scaling, DrivePath, EventQueue, Events, InputUpload, Output, OutputFormat, OutputFormatOptions, ProcessImageRequest,
	Scaling, Task,
};
use tonic::transport::Channel;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let image = std::fs::read(std::env::args().nth(1).context("No image path provided")?).context("Failed to read image")?;

	let channel = Channel::builder("http://127.0.0.1:50051".parse().unwrap())
		.connect()
		.await
		.expect("Failed to connect to image processor");

	let mut client = ImageProcessorClient::new(channel);

	let response = client
		.process_image(ProcessImageRequest {
			input_upload: Some(InputUpload {
				binary: image,
				drive_path: Some(DrivePath {
					drive: "cdn".to_string(),
					path: "test/input.avif".to_string(),
					..Default::default()
				}),
				..Default::default()
			}),
			task: Some(Task {
				events: Some(Events {
					on_cancel: Some(EventQueue {
						name: "nats".to_string(),
						topic: "image-processor.cancel".to_string(),
					}),
					on_failure: Some(EventQueue {
						name: "nats".to_string(),
						topic: "image-processor.failure".to_string(),
					}),
					on_start: Some(EventQueue {
						name: "nats".to_string(),
						topic: "image-processor.start".to_string(),
					}),
					on_success: Some(EventQueue {
						name: "nats".to_string(),
						topic: "image-processor.success".to_string(),
					}),
					..Default::default()
				}),
				output: Some(Output {
					formats: vec![OutputFormatOptions {
						format: OutputFormat::WebpAnim as i32,
						..Default::default()
					}],
					drive_path: Some(DrivePath {
						drive: "cdn".to_string(),
						path: "test/output".to_string(),
						..Default::default()
					}),
					resize: Some(Resize::Scaling(Scaling {
						base: Some(scaling::Base::BaseWidth(32)),
						scales: vec![1, 2, 3, 4],
					})),
					..Default::default()
				}),
				..Default::default()
			}),
			..Default::default()
		})
		.await
		.context("Failed to process image")?;

	println!("{:#?}", response);

	Ok(())
}
