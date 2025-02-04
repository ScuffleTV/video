use bytes::{Buf, Bytes};
use h3::quic::SendStream;
use h3::server::RequestStream;

pub async fn copy_response_body(
    mut send: RequestStream<impl SendStream<Bytes>, Bytes>,
    body: impl http_body::Body,
) -> Result<(), h3::Error> {
    let mut body = std::pin::pin!(body);
    while let Some(frame) = std::future::poll_fn(|cx| body.as_mut().poll_frame(cx)).await {
        match frame {
            Ok(frame) => match frame.into_data().map_err(|f| f.into_trailers()) {
                Ok(mut data) => send.send_data(data.copy_to_bytes(data.remaining())).await?,
                Err(Ok(trailers)) => {
                    send.send_trailers(trailers).await?;
                    return Ok(());
                }
                Err(Err(_)) => continue,
            },
            Err(_) => return Ok(()),
        }
    }

    send.finish().await?;

    Ok(())
}
