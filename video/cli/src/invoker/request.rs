pub trait RequestHandler<R> {
	type Response;

	async fn process(&mut self, req: R) -> anyhow::Result<Self::Response>;
}

macro_rules! impl_request {
    ($e:ident; $(|$self:ident, $req:ident: $req_ty:ty| -> $resp_ty:ty $action:block),* $(,)*) => {
        $(
            impl crate::invoker::request::RequestHandler<$req_ty> for $e {
                type Response = $resp_ty;

                async fn process(&mut $self, $req: $req_ty) -> anyhow::Result<Self::Response> {
                    $action
                }
            }
        )*
    };
}

use std::collections::HashMap;

pub(super) use impl_request;
use ulid::Ulid;

#[derive(Debug)]
pub struct OrganizationCreateRequest {
	pub name: String,
	pub tags: HashMap<String, String>,
}

#[derive(Debug)]
pub struct OrganizationDeleteRequest {
	pub ids: Vec<Ulid>,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct OrganizationGetRequest {
	pub ids: Vec<Ulid>,
	pub search_options: Option<pb::scuffle::video::v1::types::SearchOptions>,
}

#[derive(Debug)]
pub struct OrganizationModifyRequest {
	pub id: Ulid,
	pub name: Option<String>,
	pub tags: Option<HashMap<String, String>>,
}

#[derive(Debug)]
pub struct OrganizationTagRequest {
	pub id: Ulid,
	pub tags: HashMap<String, String>,
}

#[derive(Debug)]
pub struct OrganizationUntagRequest {
	pub id: Ulid,
	pub tags: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct Organization {
	pub id: Ulid,
	pub name: String,
	pub updated_at: chrono::DateTime<chrono::Utc>,
	#[serde(skip_serializing_if = "HashMap::is_empty")]
	pub tags: HashMap<String, String>,
}
