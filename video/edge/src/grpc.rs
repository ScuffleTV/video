use std::sync::Arc;

use tonic::transport::server::Router;

use crate::global::EdgeGlobal;

pub fn add_routes<G: EdgeGlobal, L>(_: &Arc<G>, router: Router<L>) -> Router<L> {
	router
}
