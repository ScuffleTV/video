use std::sync::Arc;

use ::utils::context::ContextExt;
use anyhow::{Context, Result};
use tonic::service::interceptor;
use tonic::transport::{Certificate, Identity, Server, ServerTlsConfig};

use crate::api::utils::auth::AuthMiddleware;
use crate::config::ApiConfig;
use crate::global::ApiGlobal;

pub(crate) mod access_token;
pub(crate) mod errors;
pub(crate) mod events;
pub(crate) mod playback_key_pair;
pub(crate) mod playback_session;
pub(crate) mod recording;
pub(crate) mod recording_config;
pub(crate) mod room;
pub(crate) mod s3_bucket;
pub(crate) mod transcoding_config;
pub(crate) mod utils;

pub use scuffle_utils::{ApiRequest, RequiredScope, ResourcePermission};

fn global_middleware<G: ApiGlobal>(
	global: &Arc<G>,
) -> impl Fn(tonic::Request<()>) -> tonic::Result<tonic::Request<()>> + Clone {
	let weak = Arc::downgrade(global);
	move |mut req: tonic::Request<()>| {
		let global = weak
			.upgrade()
			.ok_or_else(|| tonic::Status::internal("Global state was dropped"))?;

		req.extensions_mut().insert(global);

		Ok(req)
	}
}

pub async fn run<G: ApiGlobal>(global: Arc<G>) -> Result<()> {
	let config = global.config::<ApiConfig>();
	tracing::info!("API Listening on {}", config.bind_address);

	let server = if let Some(tls) = &config.tls {
		let cert = tokio::fs::read(&tls.cert).await.context("Failed to read TLS cert")?;
		let key = tokio::fs::read(&tls.key).await.context("Failed to read TLS key")?;

		let ssl = ServerTlsConfig::new().identity(Identity::from_pem(cert, key));

		let ssl = if let Some(ca_cert) = &tls.ca_cert {
			let ca_cert = tokio::fs::read(ca_cert).await.context("Failed to read CA cert")?;
			tracing::info!("API TLS enabled with client verification");
			ssl.client_ca_root(Certificate::from_pem(ca_cert))
		} else {
			tracing::info!("API TLS enabled");
			ssl
		};

		Server::builder().tls_config(ssl).context("Failed to configure TLS")?
	} else {
		tracing::info!("API TLS disabled");
		Server::builder()
	}
	.layer(interceptor::interceptor(global_middleware(&global)))
	.layer(AuthMiddleware::<G>::default())
	.add_service(room::RoomServer::<G>::build())
	.add_service(playback_key_pair::PlaybackKeyPairServer::<G>::build())
	.add_service(playback_session::PlaybackSessionServer::<G>::build())
	.add_service(recording::RecordingServer::<G>::build())
	.add_service(recording_config::RecordingConfigServer::<G>::build())
	.add_service(transcoding_config::TranscodingConfigServer::<G>::build())
	.add_service(s3_bucket::S3BucketServer::<G>::build())
	.add_service(access_token::AccessTokenServer::<G>::build())
	.add_service(events::EventsServer::<G>::build())
	.serve_with_shutdown(config.bind_address, async {
		global.ctx().done().await;
	});

	if let Ok(Err(r)) = server.context(global.ctx()).await {
		tracing::error!("API server failed: {:?}", r);
		return Err(r.into());
	}

	Ok(())
}
