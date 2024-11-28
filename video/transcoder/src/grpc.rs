use std::sync::Arc;

use tonic::transport::server::Router;

use crate::global::TranscoderGlobal;

pub fn add_routes<G: TranscoderGlobal, L>(_: &Arc<G>, router: Router<L>) -> Router<L> {
	router
}
