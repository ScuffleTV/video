use bytes::Bytes;

use super::SessionError;

#[derive(Debug, Clone)]
pub enum SessionData {
    Video { timestamp: u32, data: Bytes },
    Audio { timestamp: u32, data: Bytes },
    Amf0 { timestamp: u32, data: Bytes },
}

pub trait SessionHandler {
    fn on_publish(
        &self,
        stream_id: u32,
        app_name: &str,
        stream_name: &str,
    ) -> impl std::future::Future<Output = Result<(), SessionError>> + Send;
    fn on_unpublish(&self, stream_id: u32) -> impl std::future::Future<Output = Result<(), SessionError>> + Send;
    fn on_data(
        &self,
        stream_id: u32,
        data: SessionData,
    ) -> impl std::future::Future<Output = Result<(), SessionError>> + Send;
}
