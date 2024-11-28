use anyhow::Context as _;
use scuffle_utils::context::Context;

use self::direct::DirectBackend;
use self::grpc::GrpcBackend;
use crate::cli::Mode;

const GRPC_MODE_ERR: &str = "grpc mode requires access key id, secret access key, endpoint, and organization id";

pub mod direct;
pub mod grpc;
pub mod request;

pub struct Invoker {
	backend: InvokerBackend,
	json_output: bool,
}

enum InvokerBackend {
	Grpc(GrpcBackend),
	Direct(DirectBackend),
}

#[derive(serde::Serialize)]
struct DisplayOutput<T> {
	#[serde(rename = "__type")]
	object_type: &'static str,
	#[serde(flatten)]
	value: T,
}

impl Invoker {
	pub async fn new(context: Context, args: &crate::cli::Cli) -> anyhow::Result<Self> {
		let mode = match args.mode {
			Mode::Auto => {
				if args.access_key.is_some() && args.secret_key.is_some() && args.endpoint.is_some() {
					Mode::Grpc
				} else {
					Mode::Direct
				}
			}
			_ => args.mode,
		};

		if mode == Mode::Direct {
			return Ok(Self {
				backend: InvokerBackend::Direct(
					DirectBackend::new(context, args.config_file.clone(), args.organization_id).await?,
				),
				json_output: args.json,
			});
		}

		let Some(access_key) = args.access_key.as_ref() else {
			anyhow::bail!("{GRPC_MODE_ERR}: missing access key id")
		};

		let Some(secret_key) = args.secret_key.as_ref() else {
			anyhow::bail!("{GRPC_MODE_ERR}: missing secret access key")
		};

		let Some(endpoint) = args.endpoint.as_ref() else {
			anyhow::bail!("{GRPC_MODE_ERR}: missing endpoint")
		};

		let Some(organization_id) = args.organization_id else {
			anyhow::bail!("{GRPC_MODE_ERR}: missing organization id")
		};

		Ok(Self {
			backend: InvokerBackend::Grpc(
				GrpcBackend::new(context, access_key, secret_key, endpoint, organization_id).await?,
			),
			json_output: args.json,
		})
	}

	pub async fn invoke<R, O>(&mut self, request: R) -> anyhow::Result<O>
	where
		GrpcBackend: request::RequestHandler<R, Response = O>,
		DirectBackend: request::RequestHandler<R, Response = O>,
	{
		match &mut self.backend {
			InvokerBackend::Grpc(backend) => request::RequestHandler::<R>::process(backend, request).await,
			InvokerBackend::Direct(backend) => request::RequestHandler::<R>::process(backend, request).await,
		}
	}

	pub fn display<T: serde::Serialize>(&self, value: &T) -> anyhow::Result<()> {
		let object_type = std::any::type_name::<T>();
		let object_type = object_type.split("::").last().unwrap_or(object_type);

		let output = if self.json_output {
			serde_json::to_string_pretty(&DisplayOutput { object_type, value }).context("failed to display response")?
		} else {
			serde_yaml::to_string(&DisplayOutput { object_type, value }).context("failed to display response")?
		};

		println!("{}", output.trim());

		Ok(())
	}

	pub fn display_array<T: serde::Serialize>(&self, values: &[T]) -> anyhow::Result<()> {
		let object_type = std::any::type_name::<T>();
		let object_type = object_type.split("::").last().unwrap_or(object_type);

		let values = &values
			.iter()
			.map(|value| DisplayOutput { object_type, value })
			.collect::<Vec<_>>();

		let output = if self.json_output {
			serde_json::to_string_pretty(&values).context("failed to display response")?
		} else {
			serde_yaml::to_string(&values).context("failed to display response")?
		};

		println!("{}", output.trim());

		Ok(())
	}
}
