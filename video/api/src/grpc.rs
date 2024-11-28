use std::sync::Arc;

use tonic::transport::server::Router;

use crate::global::ApiGlobal;

pub fn add_routes<G: ApiGlobal, L>(_: &Arc<G>, router: Router<L>) -> Router<L> {
	router
}
