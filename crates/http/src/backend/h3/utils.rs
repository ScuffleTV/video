use std::fmt::Debug;

use bytes::{Buf, Bytes};
use h3::quic::SendStream;
use h3::server::RequestStream;
use http_body::Body;

use crate::service::{HttpService, HttpServiceFactory};

pub async fn copy_response_body<S, F>(
    mut send: RequestStream<S, Bytes>,
    body: <F::Service as HttpService>::ResBody,
) -> Result<(), crate::error::Error<F>>
where
    F: HttpServiceFactory,
    F::Error: std::error::Error + Debug,
    <F::Service as HttpService>::Error: std::error::Error + Debug,
    S: SendStream<Bytes>,
    <F::Service as HttpService>::ResBody: http_body::Body,
    <<F::Service as HttpService>::ResBody as http_body::Body>::Error: std::error::Error + Debug,
{
    let mut body = std::pin::pin!(body);

    while let Some(frame) = std::future::poll_fn(|cx| body.as_mut().poll_frame(cx)).await {
        match frame
            .map_err(crate::error::Error::ResBodyError)?
            .into_data()
            .map_err(|f| f.into_trailers())
        {
            Ok(mut data) => send.send_data(data.copy_to_bytes(data.remaining())).await?,
            Err(Ok(trailers)) => {
                send.send_trailers(trailers).await?;
                send.finish().await?;
                return Ok(());
            }
            Err(Err(_)) => continue,
        }
    }

    send.finish().await?;

    Ok(())
}
